//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::svgdom;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use super::super::svg_compare_layout_opts;

pub(crate) fn compare_gantt_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "structure".to_string();
    let mut check_dom: bool = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--dom-decimals" => {
                i += 1;
                dom_decimals = args.get(i).and_then(|s| s.parse::<u32>().ok()).unwrap_or(3);
            }
            "--dom-mode" => {
                i += 1;
                dom_mode = args
                    .get(i)
                    .map(|s| s.trim().to_string())
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("gantt");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("gantt");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("gantt_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("gantt");

    let mut mmd_files: Vec<PathBuf> = Vec::new();
    let Ok(entries) = fs::read_dir(&fixtures_dir) else {
        return Err(XtaskError::SvgCompareFailed(format!(
            "failed to list fixtures directory {}",
            fixtures_dir.display()
        )));
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().is_none_or(|e| e != "mmd") {
            continue;
        }

        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if matches!(
            name,
            "today_marker_and_axis.mmd"
                | "click_loose.mmd"
                | "click_strict.mmd"
                | "dateformat_hash_comment_truncates.mmd"
                | "excludes_hash_comment_truncates.mmd"
        ) {
            continue;
        }

        if let Some(ref f) = filter {
            if !name.contains(f) {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);
    let engine = merman::Engine::new();

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Gantt SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/gantt/*.svg` (Mermaid 11.12.2)\n- Local: `render_gantt_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };
        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg {}: {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::GanttDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let now_ms_override = (|| {
            let doc = roxmltree::Document::parse(&upstream_svg).ok()?;
            let x1 = doc
                .descendants()
                .filter(|n| n.has_tag_name("line"))
                .find(|n| {
                    n.attribute("class")
                        .unwrap_or_default()
                        .split_whitespace()
                        .any(|t| t == "today")
                })
                .and_then(|n| n.attribute("x1"))
                .and_then(|v| v.parse::<f64>().ok())?;
            if !x1.is_finite() {
                return None;
            }

            let min_ms = layout.tasks.iter().map(|t| t.start_ms).min()?;
            let max_ms = layout.tasks.iter().map(|t| t.end_ms).max()?;
            if max_ms <= min_ms {
                return None;
            }
            let range = (layout.width - layout.left_padding - layout.right_padding).max(1.0);

            fn gantt_today_x(
                now_ms: i64,
                min_ms: i64,
                max_ms: i64,
                range: f64,
                left_padding: f64,
            ) -> f64 {
                if max_ms <= min_ms {
                    return left_padding + (range / 2.0).round();
                }
                let t = (now_ms - min_ms) as f64 / (max_ms - min_ms) as f64;
                left_padding + (t * range).round()
            }

            let target_x = x1;
            let span = (max_ms - min_ms) as f64;
            let scaled = target_x - layout.left_padding;
            if !(span.is_finite() && scaled.is_finite() && range.is_finite()) {
                return None;
            }
            let est = (min_ms as f64) + span * (scaled / range);
            if !est.is_finite() {
                return None;
            }
            let mut lo = est.round() as i64;
            let mut hi = lo;
            let mut step: i64 = 1;

            let mut guard = 0;
            while guard < 80 {
                guard += 1;
                let x_lo = gantt_today_x(lo, min_ms, max_ms, range, layout.left_padding);
                if x_lo.is_nan() {
                    return None;
                }
                if x_lo <= target_x {
                    break;
                }
                hi = lo;
                lo = lo.saturating_sub(step);
                step = step.saturating_mul(2);
            }

            guard = 0;
            step = 1;
            while guard < 80 {
                guard += 1;
                let x_hi = gantt_today_x(hi, min_ms, max_ms, range, layout.left_padding);
                if x_hi.is_nan() {
                    return None;
                }
                if x_hi >= target_x {
                    break;
                }
                lo = hi;
                hi = hi.saturating_add(step);
                step = step.saturating_mul(2);
            }

            let x_lo = gantt_today_x(lo, min_ms, max_ms, range, layout.left_padding);
            let x_hi = gantt_today_x(hi, min_ms, max_ms, range, layout.left_padding);
            if !(x_lo <= target_x && target_x <= x_hi) {
                return None;
            }

            while lo < hi {
                let mid = lo + (hi - lo) / 2;
                let x_mid = gantt_today_x(mid, min_ms, max_ms, range, layout.left_padding);
                if x_mid < target_x {
                    lo = mid.saturating_add(1);
                } else {
                    hi = mid;
                }
            }
            let x = gantt_today_x(lo, min_ms, max_ms, range, layout.left_padding);
            if x == target_x { Some(lo) } else { None }
        })();

        let diagram_id: String = stem
            .chars()
            .map(|ch| {
                if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                    ch
                } else {
                    '_'
                }
            })
            .collect();

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            now_ms_override,
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_gantt_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        if check_dom {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}
