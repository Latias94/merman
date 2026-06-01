//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::cmd::compare::{
    DEFAULT_ROOT_DELTA_REPORT_LIMIT, RootDelta, RootDeltaReportLimit, parse_root_attrs,
    parse_root_delta_report_limit, write_root_deltas_report,
};
use crate::svgdom;
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;

use super::super::svg_compare_layout_opts;

pub(crate) fn compare_mindmap_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut report_root: bool = false;
    let mut root_report_limit = DEFAULT_ROOT_DELTA_REPORT_LIMIT;
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
            "--report-root" => report_root = true,
            "--report-root-all" => {
                report_root = true;
                root_report_limit = RootDeltaReportLimit::All;
            }
            "--report-root-limit" => {
                i += 1;
                report_root = true;
                root_report_limit = parse_root_delta_report_limit(args.get(i).map(String::as_str))?;
            }
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

    let compare_paths = crate::cmd::compare_diagram_paths("mindmap", out_path);
    let fixtures_dir = compare_paths.fixtures_dir;
    let upstream_dir = compare_paths.upstream_dir;
    let out_path = compare_paths.out_path;
    let out_svg_dir = compare_paths.out_svg_dir;
    let mmd_files = crate::cmd::list_mmd_fixtures_in_dir(&fixtures_dir, filter.as_deref(), true);

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

    let engine = merman::Engine::new();
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Mindmap SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/mindmap/*.svg` (pinned Mermaid baseline)\n- Local: `render_mindmap_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
        dom_mode, dom_decimals
    );

    let mut failures: Vec<String> = Vec::new();
    let mut root_deltas: Vec<RootDelta> = Vec::new();
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

        if should_report_root {
            match (
                parse_root_attrs(&upstream_svg),
                parse_root_attrs(&local_svg),
            ) {
                (Ok(up), Ok(lo)) => root_deltas.push(RootDelta {
                    stem: stem.to_string(),
                    max_width_delta: match (up.max_width_px, lo.max_width_px) {
                        (Some(a), Some(b)) => Some(b - a),
                        _ => None,
                    },
                    upstream: up,
                    local: lo,
                }),
                (Err(e), _) => failures.push(format!("root parse failed for upstream {stem}: {e}")),
                (_, Err(e)) => failures.push(format!("root parse failed for local {stem}: {e}")),
            }
        }

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

    if should_report_root {
        write_root_deltas_report(&mut report, &mut root_deltas[..], root_report_limit);
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
