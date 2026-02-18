//! SVG compare commands and helpers.

use crate::XtaskError;
use crate::svgdom;
use regex::Regex;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use super::svg_compare_layout_opts;

pub(crate) fn compare_er_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_markers: bool = false;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
            "--check-markers" => check_markers = true,
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
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("er");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("er");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("er_report.md")
    });

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
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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

    let re_viewbox = Regex::new(r#"viewBox="([^"]+)""#).unwrap();
    let re_max_width = Regex::new(r#"max-width:\s*([0-9.]+)px"#).unwrap();
    let re_marker_id = Regex::new(r#"<marker[^>]*\bid="([^"]+)""#).unwrap();
    let re_marker_ref = Regex::new(r#"marker-(?:start|end)="url\(#([^)]+)\)""#).unwrap();

    let mode = svgdom::DomMode::parse(&dom_mode);

    #[derive(Default)]
    struct SvgSig {
        view_box: Option<String>,
        max_width_px: Option<String>,
        marker_ids: std::collections::BTreeSet<String>,
        marker_refs: std::collections::BTreeSet<String>,
    }

    fn sig_for_svg(
        svg: &str,
        re_viewbox: &Regex,
        re_max_width: &Regex,
        re_marker_id: &Regex,
        re_marker_ref: &Regex,
    ) -> SvgSig {
        let view_box = re_viewbox
            .captures(svg)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string());
        let max_width_px = re_max_width
            .captures(svg)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string());
        let mut marker_ids = std::collections::BTreeSet::new();
        for cap in re_marker_id.captures_iter(svg) {
            if let Some(m) = cap.get(1) {
                marker_ids.insert(m.as_str().to_string());
            }
        }
        let mut marker_refs = std::collections::BTreeSet::new();
        for cap in re_marker_ref.captures_iter(svg) {
            if let Some(m) = cap.get(1) {
                marker_refs.insert(m.as_str().to_string());
            }
        }
        SvgSig {
            view_box,
            max_width_px,
            marker_ids,
            marker_refs,
        }
    }

    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1 }),
    ));
    let layout_opts = svg_compare_layout_opts();

    let mut report = String::new();
    let _ = writeln!(&mut report, "# ER SVG Compare Report");
    let _ = writeln!(&mut report);
    let _ = writeln!(
        &mut report,
        "- Upstream: `fixtures/upstream-svgs/er/*.svg` (Mermaid CLI pinned to Mermaid 11.12.2)"
    );
    let _ = writeln!(&mut report, "- Local: `render_er_diagram_svg` (Stage B)");
    let _ = writeln!(&mut report);
    let _ = writeln!(
        &mut report,
        "| fixture | markers ok | dom ok | viewBox (upstream) | viewBox (local) | max-width (upstream) | max-width (local) |"
    );
    let _ = writeln!(&mut report, "|---|---:|---:|---|---|---:|---:|");

    let mut failures: Vec<String> = Vec::new();
    let mut dom_failures: Vec<String> = Vec::new();

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
                    "missing upstream svg for {}: {} ({err})",
                    mmd_path.display(),
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

        let parsed = match futures::executor::block_on(engine.parse_diagram(
            &text,
            merman::ParseOptions {
                suppress_errors: true,
            },
        )) {
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

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::ErDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_er_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let upstream_sig = sig_for_svg(
            &upstream_svg,
            &re_viewbox,
            &re_max_width,
            &re_marker_id,
            &re_marker_ref,
        );
        let local_sig = sig_for_svg(
            &local_svg,
            &re_viewbox,
            &re_max_width,
            &re_marker_id,
            &re_marker_ref,
        );

        let mut marker_ok = true;
        let mut missing: Vec<String> = Vec::new();
        let mut extra: Vec<String> = Vec::new();
        for m in &upstream_sig.marker_ids {
            if !local_sig.marker_ids.contains(m) {
                marker_ok = false;
                missing.push(m.clone());
            }
        }
        for m in &local_sig.marker_ids {
            if !upstream_sig.marker_ids.contains(m) {
                marker_ok = false;
                extra.push(m.clone());
            }
        }
        for r in &local_sig.marker_refs {
            if !local_sig.marker_ids.contains(r) {
                marker_ok = false;
                extra.push(format!("ref-missing-def:{r}"));
            }
        }

        if check_markers && !marker_ok {
            failures.push(format!(
                "marker mismatch for {stem}: missing={:?} extra={:?}",
                missing, extra
            ));
        }

        let mut dom_ok = true;
        let dom_ok_str = if check_dom {
            let upstream_dom = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => Some(v),
                Err(err) => {
                    dom_ok = false;
                    dom_failures.push(format!("dom parse failed (upstream) for {stem}: {err}"));
                    None
                }
            };
            let local_dom = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => Some(v),
                Err(err) => {
                    dom_ok = false;
                    dom_failures.push(format!("dom parse failed (local) for {stem}: {err}"));
                    None
                }
            };

            if dom_ok {
                if let (Some(upstream_dom), Some(local_dom)) =
                    (upstream_dom.as_ref(), local_dom.as_ref())
                {
                    if let Some(diff) = svgdom::dom_diff(upstream_dom, local_dom) {
                        dom_ok = false;
                        dom_failures.push(format!("{stem}: {diff}"));
                    }
                }
            }

            if !dom_ok {
                failures.push(format!(
                    "dom mismatch for {stem} (mode={dom_mode}, decimals={dom_decimals})"
                ));
            }

            if dom_ok { "yes" } else { "no" }
        } else {
            "-"
        };

        let _ = writeln!(
            &mut report,
            "| `{}` | {} | {} | `{}` | `{}` | `{}` | `{}` |",
            stem,
            if marker_ok { "yes" } else { "no" },
            dom_ok_str,
            upstream_sig
                .view_box
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            local_sig
                .view_box
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            upstream_sig
                .max_width_px
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            local_sig
                .max_width_px
                .clone()
                .unwrap_or_else(|| "-".to_string()),
        );
    }

    if check_dom && !dom_failures.is_empty() {
        let _ = writeln!(&mut report);
        let _ = writeln!(&mut report, "## DOM Mismatch Details");
        let _ = writeln!(&mut report);
        for f in &dom_failures {
            let _ = writeln!(&mut report, "- {f}");
        }
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
        return Ok(());
    }

    Err(XtaskError::SvgCompareFailed(failures.join("\n")))
}

pub(crate) fn compare_flowchart_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut report_root: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();
    let mut text_measurer: String = "vendored".to_string();

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
            "--report-root" => report_root = true,
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

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("flowchart");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("flowchart");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("flowchart_report.md")
    });
    let out_svg_dir = out_path
        .parent()
        .unwrap_or(&workspace_root)
        .join("flowchart");

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
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
    let should_report_root = report_root || mode == svgdom::DomMode::ParityRoot;

    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1 }),
    ));
    let mut layout_opts = merman_render::LayoutOptions::default();
    if matches!(
        text_measurer.as_str(),
        "vendored" | "vendored-font" | "vendored-font-metrics"
    ) {
        layout_opts.text_measurer =
            std::sync::Arc::new(merman_render::text::VendoredFontMetricsTextMeasurer::default());
    }
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Flowchart SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/flowchart/*.svg` (Mermaid 11.12.2)\n- Local: `render_flowchart_v2_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n- Text measurer: `{}`\n",
        dom_mode, dom_decimals, text_measurer
    );

    #[derive(Debug, Clone)]
    struct RootAttrs {
        viewbox: Option<(f64, f64, f64, f64)>,
        max_width_px: Option<f64>,
    }

    fn parse_viewbox(v: &str) -> Option<(f64, f64, f64, f64)> {
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

    fn parse_style_max_width_px(style: &str) -> Option<f64> {
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

    fn parse_root_attrs(svg: &str) -> Result<RootAttrs, String> {
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

    #[derive(Debug, Clone)]
    struct RootDelta {
        stem: String,
        upstream: RootAttrs,
        local: RootAttrs,
        max_width_delta: Option<f64>,
    }

    let mut root_deltas: Vec<RootDelta> = Vec::new();

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

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

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::FlowchartV2(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_flowchart_v2_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
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

        if should_report_root {
            match (
                parse_root_attrs(&upstream_svg),
                parse_root_attrs(&local_svg),
            ) {
                (Ok(up), Ok(lo)) => {
                    let max_width_delta = match (up.max_width_px, lo.max_width_px) {
                        (Some(a), Some(b)) => Some(b - a),
                        _ => None,
                    };
                    root_deltas.push(RootDelta {
                        stem: stem.to_string(),
                        upstream: up,
                        local: lo,
                        max_width_delta,
                    });
                }
                (Err(e), _) => failures.push(format!("root parse failed for upstream {stem}: {e}")),
                (_, Err(e)) => failures.push(format!("root parse failed for local {stem}: {e}")),
            }
        }

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

    if should_report_root && !root_deltas.is_empty() {
        let _ = writeln!(
            &mut report,
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

        let take = root_deltas.len().min(25);
        let _ = writeln!(
            &mut report,
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
                &mut report,
                "| `{}` | {} | {} | {} | {} | {} |",
                d.stem, up_mw, lo_mw, mw_delta, up_vb, lo_vb
            );
        }
        let _ = writeln!(
            &mut report,
            "\nNote: These deltas are a symptom of numeric layout/text-metrics drift; matching them requires moving closer to upstream measurement behavior.\n"
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

pub(crate) fn compare_sequence_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
    let fixtures_dir = workspace_root.join("fixtures").join("sequence");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("sequence");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("sequence_report.md")
    });
    let out_svg_dir = out_path
        .parent()
        .unwrap_or(&workspace_root)
        .join("sequence");

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
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
    let parse_opts = merman::ParseOptions {
        suppress_errors: true,
    };
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Sequence SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/sequence/*.svg` (Mermaid 11.12.2)\n- Local: `render_sequence_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

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

        let parsed = match futures::executor::block_on(engine.parse_diagram(&text, parse_opts)) {
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

        let layout_opts = merman_render::LayoutOptions {
            text_measurer: std::sync::Arc::new(
                merman_render::text::VendoredFontMetricsTextMeasurer::default(),
            ),
            ..Default::default()
        };
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::SequenceDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_sequence_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
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

pub(crate) fn compare_info_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
    let fixtures_dir = workspace_root.join("fixtures").join("info");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("info");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("info_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("info");

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
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
        "# Info SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/info/*.svg` (Mermaid 11.12.2)\n- Local: `render_info_diagram_svg`\n- Mode: `{}`\n- Decimals: `{}`\n",
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

        let merman_render::model::LayoutDiagram::InfoDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_info_diagram_svg(
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

pub(crate) fn compare_pie_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
    let fixtures_dir = workspace_root.join("fixtures").join("pie");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("pie");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("pie_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("pie");

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
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
        "# Pie SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/pie/*.svg` (Mermaid 11.12.2)\n- Local: `render_pie_diagram_svg`\n- Mode: `{}`\n- Decimals: `{}`\n",
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

        let merman_render::model::LayoutDiagram::PieDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_pie_diagram_svg(
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

pub(crate) fn compare_packet_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
    let fixtures_dir = workspace_root.join("fixtures").join("packet");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("packet");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("packet_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("packet");

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
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
        "# Packet SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/packet/*.svg` (Mermaid 11.12.2)\n- Local: `render_packet_diagram_svg`\n- Mode: `{}`\n- Decimals: `{}`\n",
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

        let merman_render::model::LayoutDiagram::PacketDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_packet_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
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

pub(crate) fn compare_timeline_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
    let fixtures_dir = workspace_root.join("fixtures").join("timeline");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("timeline");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("timeline_report.md")
    });
    let out_svg_dir = out_path
        .parent()
        .unwrap_or(&workspace_root)
        .join("timeline");

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
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
        "# Timeline SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/timeline/*.svg` (Mermaid 11.12.2)\n- Local: `render_timeline_diagram_svg`\n- Mode: `{}`\n- Decimals: `{}`\n",
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

        let merman_render::model::LayoutDiagram::TimelineDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_timeline_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
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

pub(crate) fn compare_journey_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
    let fixtures_dir = workspace_root.join("fixtures").join("journey");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("journey");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("journey_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("journey");

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
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
        "# Journey SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/journey/*.svg` (Mermaid 11.12.2)\n- Local: `render_journey_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
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

        let merman_render::model::LayoutDiagram::JourneyDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_journey_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
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

pub(crate) fn compare_class_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();
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

    let mode = svgdom::DomMode::parse(&dom_mode);
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("class");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("class");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("class_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("class");

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
        if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
            n.contains("upstream_text_label_variants_spec")
                || n.contains("upstream_parser_class_spec")
        }) {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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

    let engine = merman::Engine::new();
    let layout_opts = svg_compare_layout_opts();

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# ClassDiagram SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/class/*.svg` (Mermaid 11.12.2)\n- Local: `render_class_diagram_v2_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
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
                    "missing upstream svg for {stem}: {} ({err})",
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

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::ClassDiagramV2(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let is_classdiagram_v2_header = merman::preprocess_diagram(&text, engine.registry())
            .ok()
            .map(|p| p.code.trim_start().starts_with("classDiagram-v2"))
            .unwrap_or(false);

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            aria_roledescription: is_classdiagram_v2_header.then(|| "classDiagram".to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_class_diagram_v2_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
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

pub(crate) fn compare_kanban_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
    let fixtures_dir = workspace_root.join("fixtures").join("kanban");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("kanban");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("kanban_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("kanban");

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
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
        "# Kanban SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/kanban/*.svg` (Mermaid 11.12.2)\n- Local: `render_kanban_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
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

        let merman_render::model::LayoutDiagram::KanbanDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_kanban_diagram_svg(
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

pub(crate) fn compare_gitgraph_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
    let fixtures_dir = workspace_root.join("fixtures").join("gitgraph");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("gitgraph");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("gitgraph_report.md")
    });
    let out_svg_dir = out_path
        .parent()
        .unwrap_or(&workspace_root)
        .join("gitgraph");

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
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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

    // Mermaid gitGraph auto-generates commit ids via `Math.random()`. Upstream gitGraph SVG
    // baselines in this repo are generated with a seeded renderer, so keep the local side seeded
    // too for meaningful parity-root comparisons (root viewBox/max-width depend on label widths).
    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "gitGraph": { "seed": 1 } }),
    ));

    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# GitGraph SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/gitgraph/*.svg` (Mermaid 11.12.2)\n- Local: `render_gitgraph_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
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

        let merman_render::model::LayoutDiagram::GitGraphDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_gitgraph_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
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

pub(crate) fn compare_c4_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
    let fixtures_dir = workspace_root.join("fixtures").join("c4");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("c4");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("c4_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("c4");

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
        // Keep C4 fixtures that Mermaid CLI can render (baselines exist).
        if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
            matches!(
                n,
                "nesting_updates.mmd"
                    | "upstream_boundary_spec.mmd"
                    | "upstream_c4container_header_and_direction_spec.mmd"
                    | "upstream_container_spec.mmd"
                    | "upstream_person_ext_spec.mmd"
                    | "upstream_person_spec.mmd"
                    | "upstream_system_spec.mmd"
                    | "upstream_update_element_style_all_fields_spec.mmd"
            )
        }) {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
    let parse_opts = merman::ParseOptions {
        suppress_errors: true,
    };
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# C4 SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/c4/*.svg` (Mermaid 11.12.2)\n- Local: `render_c4_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
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

        let parsed = match futures::executor::block_on(engine.parse_diagram(&text, parse_opts)) {
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

        let merman_render::model::LayoutDiagram::C4Diagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_c4_diagram_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
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

pub(crate) fn compare_block_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
    let fixtures_dir = workspace_root.join("fixtures").join("block");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("block");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("block_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("block");

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
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
        "# Block SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/block/*.svg` (Mermaid 11.12.2)\n- Local: `render_block_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

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

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
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

        let merman_render::model::LayoutDiagram::BlockDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_block_diagram_svg(
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
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
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

pub(crate) fn compare_radar_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("radar");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("radar");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("radar_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("radar");

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
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
        "# Radar SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/radar/*.svg` (Mermaid 11.12.2)\n- Local: `render_radar_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

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

        let parsed = match futures::executor::block_on(engine.parse_diagram(
            &text,
            merman::ParseOptions {
                suppress_errors: true,
            },
        )) {
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

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_layouted_svg(
            &layouted,
            layout_opts.text_measurer.as_ref(),
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

pub(crate) fn compare_treemap_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("treemap");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("treemap");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("treemap_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("treemap");

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
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
    let parse_opts = merman::ParseOptions {
        suppress_errors: true,
    };
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Treemap SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/treemap/*.svg` (Mermaid 11.12.2)\n- Local: `render_treemap_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

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

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
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

        let parsed = match futures::executor::block_on(engine.parse_diagram(&text, parse_opts)) {
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

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_layouted_svg(
            &layouted,
            layout_opts.text_measurer.as_ref(),
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
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
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

pub(crate) fn compare_requirement_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("requirement");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("requirement");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("requirement_report.md")
    });
    let out_svg_dir = out_path
        .parent()
        .unwrap_or(&workspace_root)
        .join("requirement");

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
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);
    let mut report = String::new();
    let _ = write!(
        &mut report,
        "# Requirement SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/requirement/*.svg` (Mermaid 11.12.2)\n- Local: `render_requirement_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n\n",
        dom_mode, dom_decimals
    );
    let mut failures: Vec<String> = Vec::new();

    let engine = merman::Engine::new();

    for mmd_path in &mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        if !upstream_path.is_file() {
            failures.push(format!(
                "missing upstream svg baseline for {stem}: {}",
                upstream_path.display()
            ));
            continue;
        }
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to read upstream svg for {stem} ({}): {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(mmd_path) {
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

        let diagram_id: String = stem.to_string();

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::RequirementDiagram(layout) = &layouted.layout
        else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_requirement_diagram_svg(
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
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
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
        } else if upstream_svg != local_svg {
            failures.push(format!("svg mismatch for {stem}"));
        }

        let status = if failures.iter().any(|f| f.contains(stem)) {
            "FAIL"
        } else {
            "PASS"
        };
        let _ = writeln!(
            &mut report,
            "- {status} `{stem}`\n  - fixture: `{}`\n  - upstream: `{}`\n  - local: `{}`",
            mmd_path.display(),
            upstream_path.display(),
            out_svg_dir.join(format!("{stem}.svg")).display()
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

pub(crate) fn compare_quadrantchart_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("quadrantchart");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("quadrantchart");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("quadrantchart_report.md")
    });
    let out_svg_dir = out_path
        .parent()
        .unwrap_or(&workspace_root)
        .join("quadrantchart");

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
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
                continue;
            }
        }
        mmd_files.push(path);
    }
    mmd_files.sort();

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);
    let mut report = String::new();
    let _ = write!(
        &mut report,
        "# QuadrantChart SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/quadrantchart/*.svg` (Mermaid 11.12.2)\n- Local: `render_quadrantchart_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n\n",
        dom_mode, dom_decimals
    );
    let mut failures: Vec<String> = Vec::new();

    let engine = merman::Engine::new();

    for mmd_path in &mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        if !upstream_path.is_file() {
            failures.push(format!(
                "missing upstream svg baseline for {stem}: {}",
                upstream_path.display()
            ));
            continue;
        }
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "failed to read upstream svg for {stem} ({}): {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(mmd_path) {
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

        let diagram_id: String = stem.to_string();

        let layout_opts = svg_compare_layout_opts();
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::QuadrantChartDiagram(layout) = &layouted.layout
        else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_quadrantchart_diagram_svg(
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
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
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
        } else if upstream_svg != local_svg {
            failures.push(format!("svg mismatch for {stem}"));
        }

        let status = if failures.iter().any(|f| f.contains(stem)) {
            "FAIL"
        } else {
            "PASS"
        };
        let _ = writeln!(
            &mut report,
            "- {status} `{stem}`\n  - fixture: `{}`\n  - upstream: `{}`\n  - local: `{}`",
            mmd_path.display(),
            upstream_path.display(),
            out_svg_dir.join(format!("{stem}.svg")).display()
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

pub(crate) fn compare_xychart_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("xychart");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("xychart");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("xychart_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("xychart");

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
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
        "# XYChart SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/xychart/*.svg` (Mermaid 11.12.2)\n- Local: `render_xychart_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

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

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
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

        let merman_render::model::LayoutDiagram::XyChartDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_xychart_diagram_svg(
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
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
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

pub(crate) fn compare_mindmap_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("mindmap");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("mindmap");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("mindmap_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("mindmap");

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
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
        "# Mindmap SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/mindmap/*.svg` (Mermaid 11.12.2)\n- Local: `render_mindmap_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

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

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
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

        let merman_render::model::LayoutDiagram::MindmapDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_mindmap_diagram_svg(
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
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
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

pub(crate) fn compare_sankey_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity-root".to_string();

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
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("sankey");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("sankey");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("sankey_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("sankey");

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
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
        "# Sankey SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/sankey/*.svg` (Mermaid 11.12.2)\n- Local: `render_sankey_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

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

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
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

        let merman_render::model::LayoutDiagram::SankeyDiagram(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_sankey_diagram_svg(
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
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
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

pub(crate) fn compare_architecture_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();

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
                    .unwrap_or_else(|| "parity".to_string());
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..");
    let fixtures_dir = workspace_root.join("fixtures").join("architecture");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("architecture");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("architecture_report.md")
    });
    let out_svg_dir = out_path
        .parent()
        .unwrap_or(&workspace_root)
        .join("architecture");

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
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_only_") || n.contains("_parser_only_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
        "# Architecture SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/architecture/*.svg` (Mermaid 11.12.2)\n- Local: `render_architecture_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

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

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
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

        let merman_render::model::LayoutDiagram::ArchitectureDiagram(layout) = &layouted.layout
        else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_architecture_diagram_svg(
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
                    failures.push(format!(
                        "failed to parse upstream svg dom for {stem} ({}): {err}",
                        upstream_path.display()
                    ));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!(
                        "failed to parse local svg dom for {stem} ({}): {err}",
                        local_out_path.display()
                    ));
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

pub(crate) fn compare_state_svgs(args: Vec<String>) -> Result<(), XtaskError> {
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
    let fixtures_dir = workspace_root.join("fixtures").join("state");
    let upstream_dir = workspace_root
        .join("fixtures")
        .join("upstream-svgs")
        .join("state");
    let out_path = out_path.unwrap_or_else(|| {
        workspace_root
            .join("target")
            .join("compare")
            .join("state_report.md")
    });
    let out_svg_dir = out_path.parent().unwrap_or(&workspace_root).join("state");

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
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.contains("_parser_") || n.contains("_parser_spec"))
        {
            continue;
        }
        if let Some(ref f) = filter {
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.contains(f))
            {
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
        "# StateDiagram SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/state/*.svg` (Mermaid 11.12.2)\n- Local: `render_state_diagram_v2_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
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
                    "missing upstream svg for {stem}: {} ({err})",
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

        let layout_opts = merman_render::LayoutOptions {
            text_measurer: std::sync::Arc::new(
                merman_render::text::VendoredFontMetricsTextMeasurer::default(),
            ),
            ..Default::default()
        };
        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::StateDiagramV2(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(stem.to_string()),
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_state_diagram_v2_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
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
