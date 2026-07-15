//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::cmd::compare::{
    CompareFixtureResult, CompareHarnessOptions, CompareRequest, CompareRunOptions,
    DiagramVerificationFact, ObservedRenderOperations, compare_render_environment, run_svg_compare,
    sanitize_svg_id, write_compare_result_section, write_notes_section,
    write_verification_policy_metadata,
};
use std::fmt::Write as _;

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
    let environment =
        gantt_compare_environment(&request, baseline_local_offset_minutes).map_err(|err| {
            XtaskError::SvgCompareFailed(format!("invalid Gantt baseline time: {err}"))
        })?;
    let mut observed_operations = ObservedRenderOperations::from_environment(&environment)?;
    let probe_renderer = merman::render::HeadlessRenderer::new()
        .with_engine(engine.clone())
        .with_parse_options(fact.parse_policy.options())
        .with_layout_options(layout_opts.clone())
        .with_environment(environment);
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
            &mut observed_operations,
            |_, report, _paths, options| {
                let _ = writeln!(
                    report,
                    "# {} SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/gantt/*.svg` (pinned Mermaid baseline)\n- Command: `{}`\n- Mode: `{}`\n- Decimals: `{}`\n",
                    fact.report_title, fact.command, options.dom_mode, options.dom_decimals,
                );
                write_verification_policy_metadata(report, &request, fact, options.dom_mode, false);
                report.push('\n');
            },
            |_, stem, _| {
                crate::cmd::upstream_svg_baseline_skip_reason(fact.diagram, stem)
                    .map(str::to_string)
            },
            |state, input| {
                let semantic = match probe_renderer.prepare_semantic_sync(input.text) {
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

                if prepared.family_kind() != merman::render::RenderFamilyKind::Gantt {
                    return Err(format!(
                        "unexpected render family for {}: {}",
                        input.fixture_path.display(),
                        prepared.family_kind()
                    ));
                }
                let now_ms = gantt_upstream_today_x(input.upstream_svg).and_then(|today_x| {
                    prepared
                        .gantt_time_axis_diagnostics()
                        .and_then(|diagnostics| diagnostics.unix_millis_at_rendered_x(today_x))
                });
                let svg_options = merman::render::SvgRenderOptions {
                    diagram_id: Some(sanitize_svg_id(input.stem)),
                    current_time_unix_ms: now_ms,
                    ..Default::default()
                };
                let rendered = prepared.render_svg_report(&svg_options).map_err(|err| {
                    format!("render failed for {}: {err}", input.fixture_path.display())
                })?;
                state.observe(input.stem, rendered.report())?;
                let local_svg = rendered.into_svg();

                Ok(CompareFixtureResult::Rendered {
                    local_svg,
                    compare_dom: true,
                    issues: Vec::new(),
                    notes: Vec::new(),
                })
            },
            |_, _, _| {},
            |state, report, paths, options, failures, notes| {
                state.write_report(report);
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

fn gantt_compare_environment(
    request: &CompareRequest,
    baseline_local_offset_minutes: i32,
) -> Result<merman::render::RenderEnvironment, merman::render::RenderTimeError> {
    const BASELINE_LOCAL_MIDNIGHT_UTC_MS: i64 = 1_771_113_600_000;
    let unix_ms =
        BASELINE_LOCAL_MIDNIGHT_UTC_MS - i64::from(baseline_local_offset_minutes) * 60_000;
    let snapshot = merman::render::RenderTimeSnapshot::from_unix_millis(
        unix_ms,
        baseline_local_offset_minutes,
    )?;
    Ok(compare_render_environment(request).with_time_snapshot(snapshot))
}

fn gantt_upstream_today_x(upstream_svg: &str) -> Option<f64> {
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
    x1.is_finite().then_some(x1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_environment_preserves_gantt_baseline_offset_and_root_policy() {
        let fact = super::super::diagram_verification_fact("gantt")
            .copied()
            .expect("Gantt verification fact");
        let request = CompareRequest::parse_for_fact(vec!["--no-root-overrides".to_string()], fact)
            .expect("computed root policy");

        let session = gantt_compare_environment(&request, 480)
            .expect("valid Gantt baseline environment")
            .begin_session()
            .expect("begin Gantt baseline session");

        assert_eq!(session.time().local_offset_minutes(), 480);
        assert_eq!(
            session.time().local_date(),
            chrono::NaiveDate::from_ymd_opt(2026, 2, 15).unwrap()
        );
        assert_eq!(
            session.root_viewport_override_policy(),
            merman::render::RootViewportOverridePolicy::ComputedOnly
        );
    }

    #[test]
    fn upstream_today_x_requires_an_exact_class_token_and_finite_coordinate() {
        assert_eq!(
            gantt_upstream_today_x(r#"<svg><line class="grid today active" x1="77" /></svg>"#),
            Some(77.0)
        );
        assert_eq!(
            gantt_upstream_today_x(r#"<svg><line class="not-today" x1="77" /></svg>"#),
            None
        );
        assert_eq!(
            gantt_upstream_today_x(r#"<svg><line class="today" x1="NaN" /></svg>"#),
            None
        );
        assert_eq!(gantt_upstream_today_x("<svg>"), None);
    }
}
