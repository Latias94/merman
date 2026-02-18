//! Architecture debug utilities.

use crate::XtaskError;
use crate::util::*;
use regex::Regex;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::super::svg_compare_layout_opts;

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

    let layout_opts = svg_compare_layout_opts();
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
    let layout_opts = svg_compare_layout_opts();

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
