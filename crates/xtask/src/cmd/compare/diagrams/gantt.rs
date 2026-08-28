//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::cmd::compare::{
    CompareFixtureResult, CompareHarnessOptions, CompareRequest, CompareRunFailure,
    CompareRunOptions, CompareRunResult, DiagramVerificationFact, ObservedRenderOperations,
    render_semantic_svg, run_svg_compare, sanitize_svg_id, svg_request,
    write_compare_result_section, write_notes_section, write_verification_policy_metadata,
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
        page_viewport_width: PageViewportWidthPx(
            crate::cmd::GANTT_UPSTREAM_PAGE_VIEWPORT_WIDTH_PX as f64,
        ),
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
    let dom_plan = super::super::DomComparisonPlan::from_request(&request, fact.default_dom_mode)
        .map_err(CompareRunFailure::without_evidence)?;
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
    let environment = merman::SvgEnvironment::deterministic();
    let mut observed_operations = ObservedRenderOperations::from_environment(&environment)
        .map_err(CompareRunFailure::without_evidence)?;
    let probe_renderer = merman::Renderer::new()
        .with_engine(engine.clone())
        .with_parse_options(fact.parse_policy.options());
    run_svg_compare(
        CompareHarnessOptions::new(CompareRunOptions {
            diagram: fact.diagram,
            out_path: request.out_path.clone(),
            filter: request.filter.as_deref(),
            check_dom: request.check_dom,
            dom_plan: dom_plan.clone(),
            dom_decimals,
            upstream_dom_drift_policy: request.upstream_dom_drift_policy,
        }),
        &mut observed_operations,
        |_, report, _paths, options| {
            let _ = writeln!(
                report,
                "# {} SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/gantt/*.svg` (pinned Mermaid baseline)\n- Baseline renderer: `{:?}` (page viewport: `{}`px; resolved container: `{}`px)\n- Command: `{}`\n- Modes: `{}`\n- Decimals: `{}`\n",
                fact.report_title,
                baseline_container.renderer,
                baseline_container.page_viewport_width.0,
                baseline_container.resolved_container_width().0,
                fact.command,
                options.dom_plan.label(),
                options.dom_decimals,
            );
            write_verification_policy_metadata(report, &request, fact, &options.dom_plan, false);
            report.push('\n');
        },
        |_, stem, _| {
            crate::cmd::upstream_svg_baseline_skip_reason(fact.diagram, stem).map(str::to_string)
        },
        |state, input| {
            let fixture_renderer = match input.site_config.clone() {
                Some(site_config) => probe_renderer
                    .clone()
                    .with_engine(engine.clone().with_site_config(site_config)),
                None => probe_renderer.clone(),
            };
            let semantic = match fixture_renderer
                .prepare_semantic(input.text, merman::OperationControl::new())
            {
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

            if semantic.semantic_kind() != "gantt" {
                return Err(format!(
                    "unexpected render family for {}: {}",
                    input.fixture_path.display(),
                    semantic.semantic_kind()
                ));
            }

            let rendered = render_semantic_svg(
                semantic,
                svg_request(
                    environment.clone(),
                    layout_opts.clone(),
                    Some(sanitize_svg_id(input.stem)),
                ),
            )
            .map_err(|err| format!("render failed for {}: {err}", input.fixture_path.display()))?;
            let render_evidence = state.observe(input.stem, rendered.evidence())?;
            let local_svg = rendered.svg().to_owned();

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
            write_compare_result_section(
                report,
                options.check_dom,
                failures,
                &paths.out_svg_dir,
                options.upstream_dom_drift_policy,
            );
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

pub(crate) fn gantt_baseline_runtime_policy(
    baseline_local_offset_minutes: i32,
) -> Result<merman::runtime::RuntimePolicy, merman::runtime::RuntimePolicyError> {
    let unix_ms = crate::cmd::UPSTREAM_SVG_FIXED_WALL_CLOCK_MS;
    merman::runtime::RuntimePolicy::deterministic()
        .try_with_fixed_local_offset_minutes(baseline_local_offset_minutes)
        .map(|policy| policy.with_fixed_unix_millis(unix_ms))
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
    fn baseline_runtime_policy_preserves_gantt_offset() {
        let context = gantt_baseline_runtime_policy(480)
            .expect("valid Gantt baseline runtime policy")
            .begin_operation()
            .expect("begin Gantt baseline operation");

        assert_eq!(context.local_time_zone().fixed_offset_minutes(), Some(480));
        assert_eq!(
            context.today_local(),
            merman_core::time::CivilDate::new(2024, 1, 1).unwrap()
        );
        assert_eq!(
            context.unix_millis(),
            crate::cmd::UPSTREAM_SVG_FIXED_WALL_CLOCK_MS
        );
    }
}
