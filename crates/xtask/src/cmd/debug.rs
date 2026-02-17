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

pub(crate) fn debug_svg_bbox(args: Vec<String>) -> Result<(), XtaskError> {
    let mut svg_path: Option<PathBuf> = None;
    let mut padding: f64 = 8.0;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--svg" => {
                i += 1;
                svg_path = args.get(i).map(PathBuf::from);
            }
            "--padding" => {
                i += 1;
                padding = args
                    .get(i)
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(8.0);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let svg_path = svg_path.ok_or(XtaskError::Usage)?;
    let svg = fs::read_to_string(&svg_path).map_err(|source| XtaskError::ReadFile {
        path: svg_path.display().to_string(),
        source,
    })?;

    let dbg = merman_render::svg::debug_svg_emitted_bounds(&svg).ok_or_else(|| {
        XtaskError::DebugSvgFailed(format!(
            "failed to compute emitted bounds for {}",
            svg_path.display()
        ))
    })?;

    let b = dbg.bounds;
    let vb_min_x = b.min_x - padding;
    let vb_min_y = b.min_y - padding;
    let vb_w = (b.max_x - b.min_x) + 2.0 * padding;
    let vb_h = (b.max_y - b.min_y) + 2.0 * padding;

    println!("svg: {}", svg_path.display());
    println!(
        "bounds: min=({:.6},{:.6}) max=({:.6},{:.6})",
        b.min_x, b.min_y, b.max_x, b.max_y
    );
    println!(
        "viewBox (padding={:.3}): {:.6} {:.6} {:.6} {:.6}",
        padding, vb_min_x, vb_min_y, vb_w, vb_h
    );
    println!("style max-width: {:.6}px", vb_w);

    fn print_contrib(label: &str, c: &Option<merman_render::svg::SvgEmittedBoundsContributor>) {
        let Some(c) = c else {
            println!("{label}: <none>");
            return;
        };
        fn clip_attr(s: &str) -> String {
            const MAX: usize = 140;
            if s.len() <= MAX {
                return s.to_string();
            }
            let mut out = s.chars().take(MAX).collect::<String>();
            out.push('…');
            out
        }

        println!(
            "{label}: <{} id={:?} class={:?}> bbox=({:.6},{:.6})-({:.6},{:.6})",
            c.tag, c.id, c.class, c.bounds.min_x, c.bounds.min_y, c.bounds.max_x, c.bounds.max_y
        );
        if let Some(d) = c.d.as_deref() {
            println!("  d={}", clip_attr(d));
        }
        if let Some(points) = c.points.as_deref() {
            println!("  points={}", clip_attr(points));
        }
        if let Some(tf) = c.transform.as_deref() {
            println!("  transform={}", clip_attr(tf));
        }
    }

    print_contrib("min_x", &dbg.min_x);
    print_contrib("min_y", &dbg.min_y);
    print_contrib("max_x", &dbg.max_x);
    print_contrib("max_y", &dbg.max_y);

    Ok(())
}

pub(crate) fn debug_svg_data_points(args: Vec<String>) -> Result<(), XtaskError> {
    #[derive(Debug, Clone, Copy, serde::Deserialize)]
    struct Point {
        x: f64,
        y: f64,
    }

    use base64::Engine as _;

    fn decode_points(svg: &str, element_id: &str) -> Result<Vec<Point>, XtaskError> {
        let doc = roxmltree::Document::parse(svg)
            .map_err(|e| XtaskError::SvgCompareFailed(format!("failed to parse svg xml: {e}")))?;
        let node = doc
            .descendants()
            .find(|n| n.is_element() && n.attribute("id") == Some(element_id))
            .ok_or_else(|| {
                XtaskError::DebugSvgFailed(format!("missing element with id={element_id:?}"))
            })?;
        let b64 = node.attribute("data-points").ok_or_else(|| {
            XtaskError::DebugSvgFailed(format!(
                "element id={element_id:?} has no `data-points` attribute"
            ))
        })?;

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(b64.as_bytes())
            .map_err(|e| XtaskError::DebugSvgFailed(format!("invalid base64 data-points: {e}")))?;
        let pts: Vec<Point> = serde_json::from_slice(&bytes).map_err(|e| {
            XtaskError::DebugSvgFailed(format!("invalid JSON data-points payload: {e}"))
        })?;
        Ok(pts)
    }

    let mut svg_path: Option<PathBuf> = None;
    let mut other_svg_path: Option<PathBuf> = None;
    let mut element_id: Option<String> = None;
    let mut decimals: usize = 3;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--svg" => {
                i += 1;
                svg_path = args.get(i).map(PathBuf::from);
            }
            "--other" => {
                i += 1;
                other_svg_path = args.get(i).map(PathBuf::from);
            }
            "--id" => {
                i += 1;
                element_id = args.get(i).map(|s| s.to_string());
            }
            "--decimals" => {
                i += 1;
                decimals = args
                    .get(i)
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(3);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let svg_path = svg_path.ok_or(XtaskError::Usage)?;
    let element_id = element_id.ok_or(XtaskError::Usage)?;

    let svg = fs::read_to_string(&svg_path).map_err(|source| XtaskError::ReadFile {
        path: svg_path.display().to_string(),
        source,
    })?;
    let points = decode_points(&svg, &element_id)?;

    println!("svg: {}", svg_path.display());
    println!("id: {element_id}");
    println!("points: {}", points.len());
    for (idx, p) in points.iter().enumerate() {
        println!(
            "  {idx:>3}: {x:.d$}, {y:.d$}",
            x = p.x,
            y = p.y,
            d = decimals
        );
    }

    let Some(other_svg_path) = other_svg_path else {
        return Ok(());
    };

    let other_svg = fs::read_to_string(&other_svg_path).map_err(|source| XtaskError::ReadFile {
        path: other_svg_path.display().to_string(),
        source,
    })?;
    let other_points = decode_points(&other_svg, &element_id)?;

    println!("\nother: {}", other_svg_path.display());
    println!("points: {}", other_points.len());
    if points.len() != other_points.len() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "point count mismatch: {} vs {}",
            points.len(),
            other_points.len()
        )));
    }

    println!("\ndelta (other - svg):");
    for (idx, (a, b)) in points.iter().zip(other_points.iter()).enumerate() {
        let dx = b.x - a.x;
        let dy = b.y - a.y;
        println!(
            "  {idx:>3}: dx={dx:.d$} dy={dy:.d$}",
            dx = dx,
            dy = dy,
            d = decimals
        );
    }

    Ok(())
}

pub(crate) fn debug_architecture_delta(args: Vec<String>) -> Result<(), XtaskError> {
    let mut fixture: Option<String> = None;
    let mut out_dir: Option<PathBuf> = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.trim().to_string());
            }
            "--out" => {
                i += 1;
                out_dir = args.get(i).map(PathBuf::from);
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let fixture = fixture.ok_or(XtaskError::Usage)?;
    if fixture.trim().is_empty() {
        return Err(XtaskError::Usage);
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

    type ArchPositions = (
        Option<(f64, f64, f64, f64)>,
        Option<f64>,
        BTreeMap<String, Pt>,
        BTreeMap<String, Pt>,
        BTreeMap<String, Rect>,
    );

    fn extract_arch_positions(svg: &str) -> Result<ArchPositions, XtaskError> {
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
            let Some(id) = n.attribute("id") else {
                continue;
            };

            if tag == "g"
                && id.starts_with("service-")
                && n.attribute("class")
                    .is_some_and(|c| has_class_token(c, "architecture-service"))
            {
                if let Some((x, y)) = n.attribute("transform").and_then(parse_translate) {
                    services.insert(id.to_string(), Pt { x, y });
                }
            }

            if tag == "g"
                && id.starts_with("junction-")
                && n.attribute("class")
                    .is_some_and(|c| has_class_token(c, "architecture-junction"))
            {
                if let Some((x, y)) = n.attribute("transform").and_then(parse_translate) {
                    junctions.insert(id.to_string(), Pt { x, y });
                }
            }

            if tag == "rect" && id.starts_with("group-") {
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
                groups.insert(id.to_string(), Rect { x, y, w, h });
            }
        }

        Ok((viewbox, max_width, services, junctions, groups))
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("architecture");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("architecture");
    let out_dir = out_dir.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("debug")
            .join("architecture-delta")
    });

    let mut candidates: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_file_with_extension(&path, "mmd") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if name.contains("_parser_only_") || name.contains("_parser_only_spec") {
            continue;
        }
        if name.contains(&fixture) {
            candidates.push(path);
        }
    }
    candidates.sort();

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
            XtaskError::SvgCompareFailed(format!("invalid fixture filename {}", mmd_path.display()))
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
    let parsed =
        futures::executor::block_on(engine.parse_diagram(&text, merman::ParseOptions::default()))
            .map_err(|e| {
                XtaskError::SvgCompareFailed(format!(
                    "parse failed for {}: {e}",
                    mmd_path.display()
                ))
            })?
            .ok_or_else(|| {
                XtaskError::SvgCompareFailed(format!(
                    "no diagram detected in {}",
                    mmd_path.display()
                ))
            })?;

    let layout_opts = super::svg_compare_layout_opts();
    let layouted = merman_render::layout_parsed(&parsed, &layout_opts).map_err(|e| {
        XtaskError::SvgCompareFailed(format!("layout failed for {}: {e}", mmd_path.display()))
    })?;

    let merman_render::model::LayoutDiagram::ArchitectureDiagram(layout) = &layouted.layout else {
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
    let (lo_vb, lo_mw, lo_services, lo_junctions, lo_groups) = extract_arch_positions(&local_svg)?;

    #[derive(Debug, Clone)]
    struct DeltaRow {
        id: String,
        kind: &'static str,
        up: String,
        lo: String,
        dx: f64,
        dy: f64,
        score: f64,
    }

    let mut deltas: Vec<DeltaRow> = Vec::new();

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
            score: dx.abs().max(dy.abs()),
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
            score: dx.abs().max(dy.abs()),
        });
    }

    for (id, up) in &up_groups {
        let Some(lo) = lo_groups.get(id).copied() else {
            continue;
        };
        let dx = lo.x - up.x;
        let dy = lo.y - up.y;
        deltas.push(DeltaRow {
            id: id.to_string(),
            kind: "group-rect",
            up: format!("x={:.6} y={:.6} w={:.6} h={:.6}", up.x, up.y, up.w, up.h),
            lo: format!("x={:.6} y={:.6} w={:.6} h={:.6}", lo.x, lo.y, lo.w, lo.h),
            dx,
            dy,
            score: dx.abs().max(dy.abs()),
        });
    }

    deltas.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

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
        "- local max-width(px): `{}`\n",
        lo_mw
            .map(|v| format!("{:.6}", v))
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
        "## Element deltas (top 50 by max(abs(dx), abs(dy)))\n"
    );
    let _ = writeln!(
        &mut report,
        "| kind | id | upstream | local | dx | dy | score |\n|---|---|---|---|---:|---:|---:|"
    );
    for row in deltas.iter().take(50) {
        let _ = writeln!(
            &mut report,
            "| {} | `{}` | `{}` | `{}` | {:.6} | {:.6} | {:.6} |",
            row.kind, row.id, row.up, row.lo, row.dx, row.dy, row.score
        );
    }

    fs::write(&out_report, &report).map_err(|source| XtaskError::WriteFile {
        path: out_report.display().to_string(),
        source,
    })?;

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

    type ArchSummary = (
        Option<(f64, f64, f64, f64)>,
        Option<f64>,
        BTreeMap<String, Pt>,
        BTreeMap<String, Pt>,
    );

    fn extract_arch_summary(svg: &str) -> Result<ArchSummary, XtaskError> {
        let doc = roxmltree::Document::parse(svg)
            .map_err(|e| XtaskError::SvgCompareFailed(format!("failed to parse svg xml: {e}")))?;
        let root = doc.root_element();
        let viewbox = root.attribute("viewBox").and_then(parse_viewbox);
        let max_width = root.attribute("style").and_then(parse_max_width_px);

        let mut services: BTreeMap<String, Pt> = BTreeMap::new();
        let mut junctions: BTreeMap<String, Pt> = BTreeMap::new();

        for n in doc.descendants().filter(|n| n.is_element()) {
            let tag = n.tag_name().name();
            let Some(id) = n.attribute("id") else {
                continue;
            };

            if tag == "g"
                && id.starts_with("service-")
                && n.attribute("class")
                    .is_some_and(|c| has_class_token(c, "architecture-service"))
            {
                if let Some((x, y)) = n.attribute("transform").and_then(parse_translate) {
                    services.insert(id.to_string(), Pt { x, y });
                }
            }

            if tag == "g"
                && id.starts_with("junction-")
                && n.attribute("class")
                    .is_some_and(|c| has_class_token(c, "architecture-junction"))
            {
                if let Some((x, y)) = n.attribute("transform").and_then(parse_translate) {
                    junctions.insert(id.to_string(), Pt { x, y });
                }
            }
        }

        Ok((viewbox, max_width, services, junctions))
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

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("architecture");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("architecture");
    let out_dir = out_dir.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("debug")
            .join("architecture-delta")
    });

    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let mut fixtures: Vec<PathBuf> = Vec::new();
    let entries = fs::read_dir(&fixtures_dir).map_err(|e| {
        XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}: {e}",
            fixtures_dir.display()
        ))
    })?;
    for entry in entries {
        let entry = entry.map_err(|e| {
            XtaskError::SvgCompareFailed(format!(
                "failed to read fixtures directory {}: {e}",
                fixtures_dir.display()
            ))
        })?;
        let path = entry.path();
        if path.is_file() && path.extension().is_some_and(|e| e == "mmd") {
            fixtures.push(path);
        }
    }
    fixtures.sort();

    let engine = merman::Engine::new();
    let layout_opts = super::svg_compare_layout_opts();

    #[derive(Debug, Clone)]
    struct Row {
        stem: String,
        up_vb: Option<(f64, f64, f64, f64)>,
        lo_vb: Option<(f64, f64, f64, f64)>,
        up_mw: Option<f64>,
        lo_mw: Option<f64>,
        service_center_dx: Option<f64>,
        service_center_dy: Option<f64>,
        service_mean_dx: Option<f64>,
        service_mean_dy: Option<f64>,
        junction_mean_dx: Option<f64>,
        junction_mean_dy: Option<f64>,
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

        let (up_vb, up_mw, up_services, up_junctions) = extract_arch_summary(&upstream_svg)?;
        let (lo_vb, lo_mw, lo_services, lo_junctions) = extract_arch_summary(&local_svg)?;

        let icon_size = 80.0;
        let up_center = bbox_center_from_top_left_pts(up_services.values().copied(), icon_size);
        let lo_center = bbox_center_from_top_left_pts(lo_services.values().copied(), icon_size);
        let (service_center_dx, service_center_dy) = match (up_center, lo_center) {
            (Some(up), Some(lo)) => (Some(lo.x - up.x), Some(lo.y - up.y)),
            _ => (None, None),
        };

        let svc_mean = mean_delta_by_id(&up_services, &lo_services);
        let junc_mean = mean_delta_by_id(&up_junctions, &lo_junctions);

        rows.push(Row {
            stem,
            up_vb,
            lo_vb,
            up_mw,
            lo_mw,
            service_center_dx,
            service_center_dy,
            service_mean_dx: svc_mean.map(|p| p.x),
            service_mean_dy: svc_mean.map(|p| p.y),
            junction_mean_dx: junc_mean.map(|p| p.x),
            junction_mean_dy: junc_mean.map(|p| p.y),
        });
    }

    rows.sort_by(|a, b| a.stem.cmp(&b.stem));

    let out_report = out_dir.join("architecture-delta-summary.md");
    let mut md = String::new();
    let _ = writeln!(&mut md, "# Architecture Delta Summary\n");
    let _ = writeln!(
        &mut md,
        "Generated by `xtask summarize-architecture-deltas`.\n"
    );
    let _ = writeln!(
        &mut md,
        "| fixture | up viewBox | lo viewBox | up max-width | lo max-width | svc bbox center dx | svc bbox center dy | svc mean dx | svc mean dy | junc mean dx | junc mean dy |"
    );
    let _ = writeln!(
        &mut md,
        "|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|"
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
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` |",
            r.stem,
            vb_up,
            vb_lo,
            r.up_mw
                .map(|v| format!("{:.3}", v))
                .unwrap_or_else(|| "<missing>".to_string()),
            r.lo_mw
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
        );
    }

    fs::write(&out_report, &md).map_err(|source| XtaskError::WriteFile {
        path: out_report.display().to_string(),
        source,
    })?;

    println!("report: {}", out_report.display());
    Ok(())
}

pub(crate) fn compare_dagre_layout(args: Vec<String>) -> Result<(), XtaskError> {
    use dugong::graphlib::Graph;
    use dugong::{EdgeLabel, GraphLabel, LabelPos, NodeLabel, RankDir};
    use std::collections::HashMap;

    fn rankdir_to_string(d: RankDir) -> &'static str {
        match d {
            RankDir::TB => "TB",
            RankDir::BT => "BT",
            RankDir::LR => "LR",
            RankDir::RL => "RL",
        }
    }

    fn labelpos_to_string(p: LabelPos) -> &'static str {
        match p {
            LabelPos::C => "c",
            LabelPos::L => "l",
            LabelPos::R => "r",
        }
    }

    fn snapshot_input(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> Result<JsonValue, XtaskError> {
        let opts = g.options();
        let graph = g.graph();
        let mut graph_obj = serde_json::Map::new();
        graph_obj.insert(
            "rankdir".to_string(),
            JsonValue::from(rankdir_to_string(graph.rankdir)),
        );
        graph_obj.insert("nodesep".to_string(), JsonValue::from(graph.nodesep));
        graph_obj.insert("ranksep".to_string(), JsonValue::from(graph.ranksep));
        graph_obj.insert("edgesep".to_string(), JsonValue::from(graph.edgesep));
        graph_obj.insert("marginx".to_string(), JsonValue::from(graph.marginx));
        graph_obj.insert("marginy".to_string(), JsonValue::from(graph.marginy));
        graph_obj.insert(
            "align".to_string(),
            graph
                .align
                .as_ref()
                .map(|s| JsonValue::from(s.clone()))
                .unwrap_or(JsonValue::Null),
        );
        graph_obj.insert(
            "ranker".to_string(),
            graph
                .ranker
                .as_ref()
                .map(|s| JsonValue::from(s.clone()))
                .unwrap_or(JsonValue::Null),
        );
        graph_obj.insert(
            "acyclicer".to_string(),
            graph
                .acyclicer
                .as_ref()
                .map(|s| JsonValue::from(s.clone()))
                .unwrap_or(JsonValue::Null),
        );

        let nodes = g
            .node_ids()
            .into_iter()
            .filter_map(|id| {
                let n = g.node(&id)?;
                let mut label = serde_json::Map::new();
                label.insert("width".to_string(), JsonValue::from(n.width));
                label.insert("height".to_string(), JsonValue::from(n.height));
                Some(JsonValue::Object({
                    let mut obj = serde_json::Map::new();
                    obj.insert("id".to_string(), JsonValue::from(id.clone()));
                    obj.insert(
                        "parent".to_string(),
                        g.parent(&id)
                            .map(|p| JsonValue::from(p.to_string()))
                            .unwrap_or(JsonValue::Null),
                    );
                    obj.insert("label".to_string(), JsonValue::Object(label));
                    obj
                }))
            })
            .collect::<Vec<_>>();

        let edges = g
            .edge_keys()
            .into_iter()
            .filter_map(|ek| {
                let e = g.edge_by_key(&ek)?;
                let mut label = serde_json::Map::new();
                label.insert("width".to_string(), JsonValue::from(e.width));
                label.insert("height".to_string(), JsonValue::from(e.height));
                label.insert("minlen".to_string(), JsonValue::from(e.minlen as u64));
                label.insert("weight".to_string(), JsonValue::from(e.weight));
                label.insert("labeloffset".to_string(), JsonValue::from(e.labeloffset));
                label.insert(
                    "labelpos".to_string(),
                    JsonValue::from(labelpos_to_string(e.labelpos)),
                );

                Some(JsonValue::Object({
                    let mut obj = serde_json::Map::new();
                    obj.insert("v".to_string(), JsonValue::from(ek.v.clone()));
                    obj.insert("w".to_string(), JsonValue::from(ek.w.clone()));
                    obj.insert(
                        "name".to_string(),
                        ek.name
                            .as_ref()
                            .map(|s| JsonValue::from(s.clone()))
                            .unwrap_or(JsonValue::Null),
                    );
                    obj.insert("label".to_string(), JsonValue::Object(label));
                    obj
                }))
            })
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "options": {
                "directed": opts.directed,
                "multigraph": opts.multigraph,
                "compound": opts.compound,
            },
            "graph": JsonValue::Object(graph_obj),
            "nodes": nodes,
            "edges": edges,
        }))
    }

    fn snapshot_output(
        g: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) -> Result<JsonValue, XtaskError> {
        let nodes = g
            .node_ids()
            .into_iter()
            .filter_map(|id| {
                let n = g.node(&id)?;
                Some(serde_json::json!({
                    "id": id,
                    "x": n.x,
                    "y": n.y,
                    "width": n.width,
                    "height": n.height,
                    "rank": n.rank,
                    "order": n.order,
                }))
            })
            .collect::<Vec<_>>();

        let edges = g
            .edge_keys()
            .into_iter()
            .filter_map(|ek| {
                let e = g.edge_by_key(&ek)?;
                Some(serde_json::json!({
                    "v": ek.v,
                    "w": ek.w,
                    "name": ek.name,
                    "x": e.x,
                    "y": e.y,
                    "points": e.points.iter().map(|p| serde_json::json!({"x": p.x, "y": p.y})).collect::<Vec<_>>(),
                }))
            })
            .collect::<Vec<_>>();

        Ok(serde_json::json!({
            "nodes": nodes,
            "edges": edges,
        }))
    }

    fn read_f64(v: &JsonValue) -> Option<f64> {
        match v {
            JsonValue::Number(n) => n.as_f64(),
            _ => None,
        }
    }

    fn edge_key_string(v: &str, w: &str, name: Option<&str>) -> String {
        let name = name.unwrap_or("");
        format!("{v}\u{1f}{w}\u{1f}{name}")
    }

    let mut diagram: String = "state".to_string();
    let mut fixture: Option<String> = None;
    let mut out_dir: Option<PathBuf> = None;
    let mut cluster: Option<String> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--diagram" => {
                i += 1;
                diagram = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "state".to_string());
            }
            "--fixture" => {
                i += 1;
                fixture = args.get(i).map(|s| s.to_string());
            }
            "--out-dir" => {
                i += 1;
                out_dir = args.get(i).map(PathBuf::from);
            }
            "--cluster" => {
                i += 1;
                cluster = args.get(i).map(|s| s.to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let fixture = fixture.ok_or(XtaskError::Usage)?;
    if diagram != "state" {
        return Err(XtaskError::Usage);
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join(&diagram);
    let mmd_path = fixtures_dir.join(format!("{fixture}.mmd"));
    let text = fs::read_to_string(&mmd_path).map_err(|source| XtaskError::ReadFile {
        path: mmd_path.display().to_string(),
        source,
    })?;

    let out_dir = out_dir.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("dagre-layout")
    });
    fs::create_dir_all(&out_dir).map_err(|source| XtaskError::WriteFile {
        path: out_dir.display().to_string(),
        source,
    })?;

    let engine = merman::Engine::new();
    let parsed = match futures::executor::block_on(
        engine.parse_diagram(&text, merman::ParseOptions::default()),
    ) {
        Ok(Some(v)) => v,
        Ok(None) => {
            return Err(XtaskError::DebugSvgFailed(
                "no diagram detected".to_string(),
            ));
        }
        Err(err) => return Err(XtaskError::DebugSvgFailed(format!("parse failed: {err}"))),
    };

    let measurer = merman_render::text::VendoredFontMetricsTextMeasurer::default();
    let mut g = merman_render::state::debug_build_state_diagram_v2_dagre_graph(
        &parsed.model,
        parsed.meta.effective_config.as_value(),
        &measurer,
    )
    .map_err(|e| XtaskError::DebugSvgFailed(format!("build dagre graph failed: {e}")))?;

    fn normalize_cluster_edge_endpoints_like_harness(
        graph: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
    ) {
        fn find_common_edges(
            graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
            id1: &str,
            id2: &str,
        ) -> Vec<(String, String)> {
            let edges1: Vec<(String, String)> = graph
                .edge_keys()
                .into_iter()
                .filter(|e| e.v == id1 || e.w == id1)
                .map(|e| (e.v, e.w))
                .collect();
            let edges2: Vec<(String, String)> = graph
                .edge_keys()
                .into_iter()
                .filter(|e| e.v == id2 || e.w == id2)
                .map(|e| (e.v, e.w))
                .collect();

            let edges1_prim: Vec<(String, String)> = edges1
                .into_iter()
                .map(|(v, w)| {
                    (
                        if v == id1 { id2.to_string() } else { v },
                        // Mermaid's `findCommonEdges(...)` has an asymmetry here: it maps the `w`
                        // side back to `id1` rather than `id2` (Mermaid@11.12.2).
                        if w == id1 { id1.to_string() } else { w },
                    )
                })
                .collect();

            let mut out = Vec::new();
            for e1 in edges1_prim {
                if edges2.contains(&e1) {
                    out.push(e1);
                }
            }
            out
        }

        fn find_non_cluster_child(
            graph: &Graph<NodeLabel, EdgeLabel, GraphLabel>,
            id: &str,
            cluster_id: &str,
        ) -> Option<String> {
            let children = graph.children(id);
            if children.is_empty() {
                return Some(id.to_string());
            }
            let mut reserve: Option<String> = None;
            for child in children {
                let Some(candidate) = find_non_cluster_child(graph, child, cluster_id) else {
                    continue;
                };
                let common_edges = find_common_edges(graph, cluster_id, &candidate);
                if !common_edges.is_empty() {
                    reserve = Some(candidate);
                } else {
                    return Some(candidate);
                }
            }
            reserve
        }

        let cluster_ids: Vec<String> = graph
            .node_ids()
            .into_iter()
            .filter(|id| !graph.children(id).is_empty())
            .collect();
        if cluster_ids.is_empty() {
            return;
        }

        let mut anchor: HashMap<String, String> = HashMap::new();
        for id in &cluster_ids {
            let Some(a) = find_non_cluster_child(graph, id, id) else {
                continue;
            };
            anchor.insert(id.clone(), a);
        }

        // Dagre assumes edges never touch compound nodes (nodes with children).
        //
        // Mirror `tools/dagre-harness/run.mjs` `normalizeClusterEdgeEndpoints(...)` so the Rust
        // and JS layout runs operate on the same transformed graph.
        let edge_keys = graph.edge_keys();
        for key in edge_keys {
            let mut v = key.v.clone();
            let mut w = key.w.clone();
            if cluster_ids.iter().any(|c| c == &v) {
                if let Some(a) = anchor.get(&v) {
                    v = a.clone();
                }
            }
            if cluster_ids.iter().any(|c| c == &w) {
                if let Some(a) = anchor.get(&w) {
                    w = a.clone();
                }
            }
            if v == key.v && w == key.w {
                continue;
            }

            let Some(old_label) = graph.edge_by_key(&key).cloned() else {
                continue;
            };
            let _ = graph.remove_edge_key(&key);
            graph.set_edge_named(v, w, key.name.clone(), Some(old_label));
        }
    }

    fn inject_root_cluster_node(
        g: &mut Graph<NodeLabel, EdgeLabel, GraphLabel>,
        root_id: &str,
    ) -> Result<(), XtaskError> {
        if !g.has_node(root_id) {
            g.set_node(
                root_id.to_string(),
                NodeLabel {
                    width: 1.0,
                    height: 1.0,
                    ..Default::default()
                },
            );
        }

        let node_ids: Vec<String> = g.node_ids().into_iter().collect();
        for v in node_ids {
            if v == root_id {
                continue;
            }
            if g.parent(&v).is_none() {
                g.set_parent(v, root_id.to_string());
            }
        }
        Ok(())
    }

    if let Some(cluster_id) = cluster.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let parent_label = g.graph().clone();
        let mut parent = g;
        let mut sub = merman_render::state::debug_extract_state_diagram_v2_cluster_graph(
            &mut parent,
            cluster_id,
        )
        .map_err(|e| XtaskError::DebugSvgFailed(format!("extract cluster graph failed: {e}")))?;

        // Mirror `prepare_graph(...)` overrides for extracted state subgraphs.
        sub.graph_mut().rankdir = parent_label.rankdir;
        sub.graph_mut().nodesep = parent_label.nodesep;
        sub.graph_mut().ranksep = parent_label.ranksep + 25.0;
        sub.graph_mut().edgesep = parent_label.edgesep;
        sub.graph_mut().marginx = parent_label.marginx;
        sub.graph_mut().marginy = parent_label.marginy;
        sub.graph_mut().align = parent_label.align;
        sub.graph_mut().ranker = parent_label.ranker;
        sub.graph_mut().acyclicer = parent_label.acyclicer;

        inject_root_cluster_node(&mut sub, cluster_id)?;
        g = sub;
    }

    // Mirror the JS dagre harness normalization for compound-edge endpoints so the input graph is
    // identical for both the JS and Rust layout runs.
    normalize_cluster_edge_endpoints_like_harness(&mut g);

    let input_path = out_dir.join(format!("{fixture}.input.json"));
    let js_path = out_dir.join(format!("{fixture}.js.json"));
    let rust_path = out_dir.join(format!("{fixture}.rust.json"));

    let input = snapshot_input(&g)?;
    fs::write(&input_path, serde_json::to_string_pretty(&input)?).map_err(|source| {
        XtaskError::WriteFile {
            path: input_path.display().to_string(),
            source,
        }
    })?;

    let script_path = workspace_root
        .join("tools")
        .join("dagre-harness")
        .join("run.mjs");

    let status = Command::new("node")
        .arg(&script_path)
        .arg("--in")
        .arg(&input_path)
        .arg("--out")
        .arg(&js_path)
        .status()
        .map_err(|e| XtaskError::DebugSvgFailed(format!("failed to spawn node: {e}")))?;
    if !status.success() {
        return Err(XtaskError::DebugSvgFailed(format!(
            "node dagre harness failed (exit={})",
            status.code().unwrap_or(-1)
        )));
    }

    let js_raw = fs::read_to_string(&js_path).map_err(|source| XtaskError::ReadFile {
        path: js_path.display().to_string(),
        source,
    })?;
    let js_out: JsonValue = serde_json::from_str(&js_raw)?;

    dugong::layout_dagreish(&mut g);
    let rust_out = snapshot_output(&g)?;
    fs::write(&rust_path, serde_json::to_string_pretty(&rust_out)?).map_err(|source| {
        XtaskError::WriteFile {
            path: rust_path.display().to_string(),
            source,
        }
    })?;

    let mut js_nodes: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    if let Some(arr) = js_out.get("nodes").and_then(|v| v.as_array()) {
        for n in arr {
            let Some(id) = n.get("id").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(label) = n.get("label").and_then(|v| v.as_object()) else {
                continue;
            };
            let Some(x) = label.get("x").and_then(read_f64) else {
                continue;
            };
            let Some(y) = label.get("y").and_then(read_f64) else {
                continue;
            };
            js_nodes.insert(id.to_string(), (x, y));
        }
    }

    let mut js_edges: BTreeMap<String, Vec<(f64, f64)>> = BTreeMap::new();
    if let Some(arr) = js_out.get("edges").and_then(|v| v.as_array()) {
        for e in arr {
            let Some(v) = e.get("v").and_then(|v| v.as_str()) else {
                continue;
            };
            let Some(w) = e.get("w").and_then(|v| v.as_str()) else {
                continue;
            };
            let name = e.get("name").and_then(|v| v.as_str());
            let key = edge_key_string(v, w, name);
            let Some(label) = e.get("label").and_then(|v| v.as_object()) else {
                continue;
            };
            let Some(points) = label.get("points").and_then(|v| v.as_array()) else {
                continue;
            };
            let mut pts: Vec<(f64, f64)> = Vec::new();
            for p in points {
                let Some(px) = p.get("x").and_then(read_f64) else {
                    continue;
                };
                let Some(py) = p.get("y").and_then(read_f64) else {
                    continue;
                };
                pts.push((px, py));
            }
            js_edges.insert(key, pts);
        }
    }

    let mut max_node_delta = 0.0f64;
    let mut max_node_id: Option<String> = None;

    for id in g.node_ids() {
        let Some(n) = g.node(&id) else {
            continue;
        };
        let (Some(rx), Some(ry)) = (n.x, n.y) else {
            continue;
        };
        let Some((jx, jy)) = js_nodes.get(&id) else {
            continue;
        };
        let dx = jx - rx;
        let dy = jy - ry;
        let d = dx.abs().max(dy.abs());
        if d > max_node_delta {
            max_node_delta = d;
            max_node_id = Some(id);
        }
    }

    let mut max_edge_delta = 0.0f64;
    let mut max_edge_id: Option<String> = None;

    for ek in g.edge_keys() {
        let Some(e) = g.edge_by_key(&ek) else {
            continue;
        };
        let key = edge_key_string(&ek.v, &ek.w, ek.name.as_deref());
        let Some(jpts) = js_edges.get(&key) else {
            continue;
        };
        if e.points.len() != jpts.len() {
            max_edge_delta = f64::INFINITY;
            max_edge_id = Some(key);
            break;
        }
        for (rp, (jx, jy)) in e.points.iter().zip(jpts.iter()) {
            let dx = jx - rp.x;
            let dy = jy - rp.y;
            let d = dx.abs().max(dy.abs());
            if d > max_edge_delta {
                max_edge_delta = d;
                max_edge_id = Some(key.clone());
            }
        }
    }

    println!("diagram: {diagram}");
    println!("fixture: {fixture}");
    println!("input:   {}", input_path.display());
    println!("js:      {}", js_path.display());
    println!("rust:    {}", rust_path.display());
    println!(
        "max node delta: {:.6} (node={})",
        max_node_delta,
        max_node_id.as_deref().unwrap_or("<none>")
    );
    println!(
        "max edge delta: {:.6} (edge={})",
        max_edge_delta,
        max_edge_id.as_deref().unwrap_or("<none>")
    );

    Ok(())
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

pub(crate) fn debug_mindmap_svg_positions(args: Vec<String>) -> Result<(), XtaskError> {
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
            .join("mindmap")
            .join(format!("{f}.svg"));
        let local_default = workspace_root
            .join("target")
            .join("compare")
            .join("mindmap")
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
    struct RootInfo {
        view_box: Option<String>,
        max_width: Option<String>,
    }

    #[derive(Debug, Clone)]
    struct NodePos {
        id: String,
        class: String,
        x: f64,
        y: f64,
    }

    fn parse_root_info(svg: &str) -> Result<RootInfo, String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;
        let root = doc.root_element();
        let view_box = root.attribute("viewBox").map(|s| s.to_string());
        let max_width = root.attribute("style").and_then(|s| {
            static RE: OnceLock<Regex> = OnceLock::new();
            let re = RE.get_or_init(|| Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap());
            re.captures(s)
                .and_then(|c| c.get(1).map(|m| m.as_str().to_string()))
        });
        Ok(RootInfo {
            view_box,
            max_width,
        })
    }

    fn parse_node_positions(svg: &str) -> Result<Vec<NodePos>, String> {
        let doc = roxmltree::Document::parse(svg).map_err(|e| e.to_string())?;
        let mut out: Vec<NodePos> = Vec::new();

        for n in doc.descendants().filter(|n| n.is_element()) {
            if n.tag_name().name() != "g" {
                continue;
            }
            let Some(id) = n.attribute("id") else {
                continue;
            };
            if !id.starts_with("node_") {
                continue;
            }
            let Some(class) = n.attribute("class") else {
                continue;
            };
            if !class.split_whitespace().any(|t| t == "node") {
                continue;
            }
            let Some(transform) = n.attribute("transform") else {
                continue;
            };
            let Some(local) = parse_translate(transform) else {
                continue;
            };
            let abs = accumulated_translate(n);
            out.push(NodePos {
                id: id.to_string(),
                class: class.to_string(),
                x: local.x + abs.x,
                y: local.y + abs.y,
            });
        }

        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    let up_root = parse_root_info(&upstream_svg).map_err(XtaskError::DebugSvgFailed)?;
    let lo_root = parse_root_info(&local_svg).map_err(XtaskError::DebugSvgFailed)?;
    let up_nodes = parse_node_positions(&upstream_svg).map_err(XtaskError::DebugSvgFailed)?;
    let lo_nodes = parse_node_positions(&local_svg).map_err(XtaskError::DebugSvgFailed)?;

    println!("upstream: {}", upstream_path.display());
    println!("local:    {}", local_path.display());
    println!();

    println!("== Root SVG ==");
    println!(
        "upstream viewBox: {:?}",
        up_root.view_box.as_deref().unwrap_or("<missing>")
    );
    println!(
        "local    viewBox: {:?}",
        lo_root.view_box.as_deref().unwrap_or("<missing>")
    );
    println!(
        "upstream max-width(px): {:?}",
        up_root.max_width.as_deref().unwrap_or("<missing>")
    );
    println!(
        "local    max-width(px): {:?}",
        lo_root.max_width.as_deref().unwrap_or("<missing>")
    );
    println!();

    println!("== Nodes ==");
    println!("upstream nodes: {}", up_nodes.len());
    println!("local nodes:    {}", lo_nodes.len());
    println!();

    let mut up_by_id: std::collections::BTreeMap<&str, &NodePos> =
        std::collections::BTreeMap::new();
    for n in &up_nodes {
        up_by_id.insert(n.id.as_str(), n);
    }
    let mut lo_by_id: std::collections::BTreeMap<&str, &NodePos> =
        std::collections::BTreeMap::new();
    for n in &lo_nodes {
        lo_by_id.insert(n.id.as_str(), n);
    }

    for (id, up) in &up_by_id {
        let lo = lo_by_id.get(id).copied();
        match lo {
            Some(lo) => {
                if up.x != lo.x || up.y != lo.y || up.class != lo.class {
                    println!("id={id}");
                    println!("  upstream: ({:.6}, {:.6}) class={}", up.x, up.y, up.class);
                    println!("  local:    ({:.6}, {:.6}) class={}", lo.x, lo.y, lo.class);
                }
            }
            None => println!("upstream-only: {id}"),
        }
    }
    for id in lo_by_id.keys() {
        if !up_by_id.contains_key(id) {
            println!("local-only: {id}");
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
