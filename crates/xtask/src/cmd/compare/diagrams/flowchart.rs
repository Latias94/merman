//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::svgdom;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

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
