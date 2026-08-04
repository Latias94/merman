//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::cmd::compare::{
    CompareFixtureResult, CompareHarnessOptions, CompareRequest, CompareRunFailure,
    CompareRunOptions, CompareRunResult, DiagramVerificationFact, ObservedRenderOperations,
    run_svg_compare, sanitize_svg_id, write_compare_result_section, write_notes_section,
    write_verification_policy_metadata,
};
use std::fmt::Write as _;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GanttBaselineRenderer {
    MermaidCli,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PageViewportWidthPx(f64);

#[derive(Debug, Clone, Copy, PartialEq)]
struct ContainerWidthPx(f64);

#[derive(Debug, Clone, Copy, PartialEq)]
struct GanttBaselineContainerProfile {
    renderer: GanttBaselineRenderer,
    page_viewport_width: PageViewportWidthPx,
    body_margin_inline_start_px: f64,
    body_margin_inline_end_px: f64,
}

impl GanttBaselineContainerProfile {
    const MERMAID_CLI: Self = Self {
        renderer: GanttBaselineRenderer::MermaidCli,
        page_viewport_width: PageViewportWidthPx(1_200.0),
        body_margin_inline_start_px: 8.0,
        body_margin_inline_end_px: 8.0,
    };

    fn resolved_container_width(self) -> ContainerWidthPx {
        ContainerWidthPx(
            self.page_viewport_width.0
                - self.body_margin_inline_start_px
                - self.body_margin_inline_end_px,
        )
    }

    fn layout_options(self) -> merman::svg::LayoutOptions {
        let mut options = crate::cmd::svg_compare_layout_opts();
        options.container_width = self.resolved_container_width().0;
        options
    }
}

pub(super) fn compare_gantt_args(
    fact: DiagramVerificationFact,
    args: Vec<String>,
) -> Result<(), XtaskError> {
    let request = CompareRequest::parse_for_fact(args, fact)?;
    compare_gantt_request(fact, request)
        .map(|_| ())
        .map_err(CompareRunFailure::into_error)
}

pub(super) fn compare_gantt_request(
    fact: DiagramVerificationFact,
    request: CompareRequest,
) -> CompareRunResult {
    let dom_mode = request.dom_mode.as_deref().unwrap_or(fact.default_dom_mode);
    let dom_decimals = request.dom_decimals.unwrap_or(3);

    // Mermaid Gantt uses JavaScript local-time semantics. Upstream SVG baselines are therefore
    // timezone-dependent unless the renderer is pinned. Our fixture corpus was generated under
    // a fixed UTC+08:00 environment, so pin the local offset here to keep CI deterministic across
    // runners.
    //
    // Override via `MERMAN_GANTT_BASELINE_LOCAL_OFFSET_MINUTES` if the baseline corpus is ever
    // regenerated under a different timezone.
    let baseline_local_offset_minutes = gantt_baseline_local_offset_minutes();

    let runtime_policy = gantt_baseline_runtime_policy(baseline_local_offset_minutes)
        .map_err(|err| XtaskError::SvgCompareFailed(format!("invalid Gantt baseline time: {err}")))
        .map_err(CompareRunFailure::without_evidence)?;
    let engine = crate::cmd::svg_compare_engine().with_runtime_policy(runtime_policy.clone());
    let baseline_container = GanttBaselineContainerProfile::MERMAID_CLI;
    let layout_opts = baseline_container.layout_options();
    let environment =
        merman::svg::RenderEnvironment::deterministic().with_runtime_policy(runtime_policy.clone());
    let mut observed_operations = ObservedRenderOperations::from_environment(&environment)
        .map_err(CompareRunFailure::without_evidence)?;
    let probe_renderer = merman::svg::HeadlessRenderer::new()
        .with_engine(engine.clone())
        .with_parse_options(fact.parse_policy.options())
        .with_layout_options(layout_opts.clone())
        .with_environment(environment);
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
                "# {} SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/gantt/*.svg` (pinned Mermaid baseline)\n- Baseline renderer: `{:?}` (page viewport: `{}`px; resolved container: `{}`px)\n- Command: `{}`\n- Mode: `{}`\n- Decimals: `{}`\n",
                fact.report_title,
                baseline_container.renderer,
                baseline_container.page_viewport_width.0,
                baseline_container.resolved_container_width().0,
                fact.command,
                options.dom_mode,
                options.dom_decimals,
            );
            write_verification_policy_metadata(report, &request, fact, options.dom_mode, false);
            report.push('\n');
        },
        |_, stem, _| {
            crate::cmd::upstream_svg_baseline_skip_reason(fact.diagram, stem).map(str::to_string)
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

            if prepared.family_kind() != merman::svg::RenderFamilyKind::Gantt {
                return Err(format!(
                    "unexpected render family for {}: {}",
                    input.fixture_path.display(),
                    prepared.family_kind()
                ));
            }
            let prepared = if let Some(runtime_policy) = gantt_calibrated_runtime_policy(
                &prepared,
                input.upstream_svg,
                baseline_local_offset_minutes,
            )
            .map_err(|err| format!("invalid calibrated Gantt baseline time: {err}"))?
            {
                let renderer = merman::svg::HeadlessRenderer::new()
                    .with_engine(engine.clone())
                    .with_parse_options(fact.parse_policy.options())
                    .with_layout_options(layout_opts.clone())
                    .with_environment(
                        merman::svg::RenderEnvironment::deterministic()
                            .with_runtime_policy(runtime_policy),
                    );
                let semantic = renderer
                    .prepare_semantic_sync(input.text)
                    .map_err(|err| {
                        format!(
                            "calibrated parse failed for {}: {err}",
                            input.fixture_path.display()
                        )
                    })?
                    .ok_or_else(|| {
                        format!(
                            "calibrated parse detected no diagram in {}",
                            input.fixture_path.display()
                        )
                    })?;
                semantic.continue_layout().map_err(|err| {
                    format!(
                        "calibrated layout failed for {}: {err}",
                        input.fixture_path.display()
                    )
                })?
            } else {
                prepared
            };
            let svg_options = merman::svg::SvgRenderOptions {
                diagram_id: Some(sanitize_svg_id(input.stem)),
                ..Default::default()
            };
            let rendered = prepared.render_svg_report(&svg_options).map_err(|err| {
                format!("render failed for {}: {err}", input.fixture_path.display())
            })?;
            let render_evidence = state.observe(input.stem, rendered.report())?;
            let local_svg = rendered.into_svg();

            Ok(CompareFixtureResult::Rendered {
                render_evidence,
                local_svg,
                compare_dom: true,
                issues: Vec::new(),
                notes: Vec::new(),
            })
        },
        |_, _, _| {},
        |state, report, paths, options, failures, notes| {
            state.write_report(report);
            write_compare_result_section(report, options.check_dom, failures, &paths.out_svg_dir);
            write_notes_section(report, notes);
        },
    )
}

pub(crate) fn gantt_baseline_local_offset_minutes() -> i32 {
    std::env::var("MERMAN_GANTT_BASELINE_LOCAL_OFFSET_MINUTES")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(480)
}

pub(crate) fn gantt_compare_environment(
    baseline_local_offset_minutes: i32,
) -> Result<merman::svg::RenderEnvironment, merman::runtime::RuntimePolicyError> {
    Ok(
        merman::svg::RenderEnvironment::deterministic().with_runtime_policy(
            gantt_baseline_runtime_policy(baseline_local_offset_minutes)?,
        ),
    )
}

fn gantt_baseline_runtime_policy(
    baseline_local_offset_minutes: i32,
) -> Result<merman::runtime::RuntimePolicy, merman::runtime::RuntimePolicyError> {
    const BASELINE_LOCAL_MIDNIGHT_UTC_MS: i64 = 1_771_113_600_000;
    let unix_ms =
        BASELINE_LOCAL_MIDNIGHT_UTC_MS - i64::from(baseline_local_offset_minutes) * 60_000;
    merman::runtime::RuntimePolicy::deterministic()
        .try_with_fixed_local_offset_minutes(baseline_local_offset_minutes)
        .map(|policy| policy.with_fixed_unix_millis(unix_ms))
}

pub(crate) fn gantt_calibrated_runtime_policy(
    prepared: &merman::svg::PreparedRender,
    upstream_svg: &str,
    baseline_local_offset_minutes: i32,
) -> Result<Option<merman::runtime::RuntimePolicy>, merman::runtime::RuntimePolicyError> {
    let now_ms = gantt_upstream_today_x(upstream_svg).and_then(|today_x| {
        prepared
            .gantt_time_axis_diagnostics()
            .and_then(|diagnostics| diagnostics.unix_millis_at_rendered_x(today_x))
    });
    now_ms
        .map(|unix_ms| {
            merman::runtime::RuntimePolicy::deterministic()
                .try_with_fixed_local_offset_minutes(baseline_local_offset_minutes)
                .map(|policy| policy.with_fixed_unix_millis(unix_ms))
        })
        .transpose()
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
    fn mmdc_baseline_resolves_1200_page_viewport_to_1184_container() {
        let profile = GanttBaselineContainerProfile::MERMAID_CLI;

        assert_eq!(profile.page_viewport_width, PageViewportWidthPx(1_200.0));
        assert_eq!(
            profile.resolved_container_width(),
            ContainerWidthPx(1_184.0)
        );
        assert_eq!(profile.layout_options().container_width, 1_184.0);
    }

    #[test]
    fn compare_environment_preserves_gantt_baseline_offset() {
        let session = gantt_compare_environment(480)
            .expect("valid Gantt baseline environment")
            .begin_session()
            .expect("begin Gantt baseline session");

        assert_eq!(session.local_time_zone().fixed_offset_minutes(), Some(480));
        assert_eq!(
            session.local_date(),
            merman_core::time::CivilDate::new(2026, 2, 15).unwrap()
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
