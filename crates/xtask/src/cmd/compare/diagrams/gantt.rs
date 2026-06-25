//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::cmd::compare::{
    CompareFixtureResult, CompareRunOptions, run_svg_compare, sanitize_svg_id,
    write_compare_result_section, write_notes_section,
};
use std::fmt::Write as _;
use std::path::PathBuf;

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

    // Mermaid Gantt uses JavaScript local-time semantics. Upstream SVG baselines are therefore
    // timezone-dependent unless the renderer is pinned. Our fixture corpus was generated under
    // a fixed UTC+08:00 environment, so pin the local offset here to keep CI deterministic across
    // runners.
    //
    // Override via `MERMAN_GANTT_BASELINE_LOCAL_OFFSET_MINUTES` if the baseline corpus is ever
    // regenerated under a different timezone.
    let baseline_local_offset_minutes: i32 =
        std::env::var("MERMAN_GANTT_BASELINE_LOCAL_OFFSET_MINUTES")
            .ok()
            .and_then(|v| v.parse::<i32>().ok())
            .unwrap_or(480);

    let engine = crate::cmd::svg_compare_engine()
        .with_fixed_local_offset_minutes(Some(baseline_local_offset_minutes));
    let layout_opts = crate::cmd::svg_compare_layout_opts();

    merman::time::with_fixed_local_offset_minutes(Some(baseline_local_offset_minutes), || {
        run_svg_compare(
            CompareRunOptions {
                diagram: "gantt",
                out_path,
                filter: filter.as_deref(),
                check_dom,
                dom_mode: &dom_mode,
                dom_decimals,
            },
            &mut (),
            |_, report, _paths, options| {
                let _ = writeln!(
                    report,
                    "# Gantt SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/gantt/*.svg` (pinned Mermaid baseline)\n- Local: `render_gantt_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
                    options.dom_mode, options.dom_decimals
                );
            },
            |_, stem, _| {
                gantt_fixture_is_excluded(stem)
                    .then_some("excluded from deterministic Gantt SVG compare".to_string())
            },
            |_, input| {
                let parsed = match futures::executor::block_on(
                    engine.parse_diagram(input.text, merman::ParseOptions::default()),
                ) {
                    Ok(Some(v)) => v,
                    Ok(None) => {
                        return Err(format!(
                            "no diagram detected in {}",
                            input.fixture_path.display()
                        ));
                    }
                    Err(err) => {
                        return Err(format!(
                            "parse failed for {}: {err}",
                            input.fixture_path.display()
                        ));
                    }
                };

                let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
                    Ok(v) => v,
                    Err(err) => {
                        return Err(format!(
                            "layout failed for {}: {err}",
                            input.fixture_path.display()
                        ));
                    }
                };

                let merman_render::model::LayoutDiagram::GanttDiagram(layout) = &layouted.layout
                else {
                    return Err(format!(
                        "unexpected layout type for {}: {}",
                        input.fixture_path.display(),
                        layouted.meta.diagram_type
                    ));
                };

                let svg_opts = merman_render::svg::SvgRenderOptions {
                    diagram_id: Some(sanitize_svg_id(input.stem)),
                    now_ms_override: gantt_now_ms_override(input.upstream_svg, layout),
                    ..Default::default()
                };

                let local_svg = merman_render::svg::render_gantt_diagram_svg(
                    layout,
                    &layouted.semantic,
                    &layouted.meta.effective_config,
                    &svg_opts,
                )
                .map_err(|err| {
                    format!("render failed for {}: {err}", input.fixture_path.display())
                })?;

                Ok(CompareFixtureResult::Rendered {
                    local_svg,
                    compare_dom: true,
                    issues: Vec::new(),
                    notes: Vec::new(),
                })
            },
            |_, report, paths, options, failures, notes| {
                write_compare_result_section(
                    report,
                    options.check_dom,
                    failures,
                    &paths.out_svg_dir,
                );
                write_notes_section(report, notes);
            },
        )
    })
}

fn gantt_fixture_is_excluded(stem: &str) -> bool {
    matches!(
        stem,
        "today_marker_and_axis"
            | "click_loose"
            | "click_strict"
            | "dateformat_hash_comment_truncates"
            | "excludes_hash_comment_truncates"
    )
}

fn gantt_now_ms_override(
    upstream_svg: &str,
    layout: &merman_render::model::GanttDiagramLayout,
) -> Option<i64> {
    let doc = roxmltree::Document::parse(upstream_svg).ok()?;
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
}

fn gantt_today_x(now_ms: i64, min_ms: i64, max_ms: i64, range: f64, left_padding: f64) -> f64 {
    if max_ms <= min_ms {
        return left_padding + (range / 2.0).round();
    }
    let t = (now_ms - min_ms) as f64 / (max_ms - min_ms) as f64;
    left_padding + (t * range).round()
}
