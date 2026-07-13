//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::cmd::compare::{
    CompareFixtureResult, CompareHarnessOptions, CompareRequest, CompareRunOptions,
    DiagramVerificationFact, run_svg_compare, sanitize_svg_id, write_compare_result_section,
    write_notes_section, write_verification_policy_metadata,
};
use std::fmt::Write as _;

pub(crate) fn compare_gantt_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let fact = super::diagram_verification_fact("gantt")
        .copied()
        .expect("Gantt must have a verification fact");
    compare_gantt_args(fact, args)
}

pub(super) fn compare_gantt_args(
    fact: DiagramVerificationFact,
    args: Vec<String>,
) -> Result<(), XtaskError> {
    let request = CompareRequest::parse_for_fact(args, fact)?;
    compare_gantt_request(fact, request)
}

pub(super) fn compare_gantt_request(
    fact: DiagramVerificationFact,
    request: CompareRequest,
) -> Result<(), XtaskError> {
    let dom_mode = request.dom_mode.as_deref().unwrap_or(fact.default_dom_mode);
    let dom_decimals = request.dom_decimals.unwrap_or(3);

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
            CompareHarnessOptions::new(CompareRunOptions {
                diagram: fact.diagram,
                out_path: request.out_path.clone(),
                filter: request.filter.as_deref(),
                check_dom: request.check_dom,
                dom_mode,
                dom_decimals,
            }),
            &mut (),
            |_, report, _paths, options| {
                let _ = writeln!(
                    report,
                    "# {} SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/gantt/*.svg` (pinned Mermaid baseline)\n- Render path: `{}`\n- Command: `{}`\n- Mode: `{}`\n- Decimals: `{}`\n",
                    fact.report_title,
                    fact.render_path().label(),
                    fact.command,
                    options.dom_mode,
                    options.dom_decimals
                );
                write_verification_policy_metadata(report, &request, fact, options.dom_mode, false);
                report.push('\n');
            },
            |_, stem, _| {
                crate::cmd::upstream_svg_baseline_skip_reason(fact.diagram, stem)
                    .map(str::to_string)
            },
            |_, input| {
                let semantic = match merman::render::prepare_semantic_sync(
                    &engine,
                    input.text,
                    fact.parse_policy.options(),
                    &layout_opts,
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

                let prepared = match semantic.continue_layout() {
                    Ok(v) => v,
                    Err(err) => {
                        return Err(format!(
                            "layout failed for {}: {err}",
                            input.fixture_path.display()
                        ));
                    }
                };

                let merman_render::model::LayoutDiagram::GanttDiagram(layout) = prepared.layout()
                else {
                    return Err(format!(
                        "unexpected layout type for {}: {}",
                        input.fixture_path.display(),
                        prepared.metadata().diagram_type
                    ));
                };
                let now_ms_override = gantt_now_ms_override(input.upstream_svg, layout);

                let svg_opts = merman_render::svg::SvgRenderOptions {
                    diagram_id: Some(sanitize_svg_id(input.stem)),
                    now_ms_override,
                    ..Default::default()
                };

                let local_svg = prepared.render_svg(&svg_opts).map_err(|err| {
                    format!("render failed for {}: {err}", input.fixture_path.display())
                })?;

                Ok(CompareFixtureResult::Rendered {
                    local_svg,
                    compare_dom: true,
                    issues: Vec::new(),
                    notes: Vec::new(),
                })
            },
            |_, _, _| {},
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
