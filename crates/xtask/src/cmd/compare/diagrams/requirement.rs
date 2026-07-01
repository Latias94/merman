//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::cmd::compare::{
    CompareFixtureReportInput, CompareFixtureResult, CompareRunOptions,
    run_svg_compare_with_fixture_reports,
};
use std::fmt::Write as _;
use std::path::PathBuf;

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

    let engine = crate::cmd::svg_compare_engine();
    let layout_opts = crate::cmd::svg_compare_layout_opts();
    run_svg_compare_with_fixture_reports(
        CompareRunOptions {
            diagram: "requirement",
            out_path,
            filter: filter.as_deref(),
            check_dom,
            dom_mode: &dom_mode,
            dom_decimals,
        },
        &mut (),
        |_, report, _paths, options| {
            let _ = write!(
                report,
                "# Requirement SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/requirement/*.svg` (pinned Mermaid baseline)\n- Local: `render_requirement_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n\n",
                options.dom_mode, options.dom_decimals
            );
        },
        |_, _, _| None,
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

            let merman_render::model::LayoutDiagram::RequirementDiagram(layout) = &layouted.layout
            else {
                return Err(format!(
                    "unexpected layout type for {}: {}",
                    input.fixture_path.display(),
                    layouted.meta.diagram_type
                ));
            };

            let svg_opts = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(input.stem.to_string()),
                ..Default::default()
            };

            let local_svg = merman_render::svg::render_requirement_diagram_svg(
                layout,
                &layouted.semantic,
                &layouted.meta.effective_config,
                layouted.meta.title.as_deref(),
                &svg_opts,
            )
            .map_err(|err| format!("render failed for {}: {err}", input.fixture_path.display()))?;

            Ok(CompareFixtureResult::RenderedWithPolicy {
                local_svg,
                compare_dom: true,
                compare_svg_when_dom_disabled: true,
                issues: Vec::new(),
                notes: Vec::new(),
            })
        },
        |_, report, fixture| write_status_line(report, fixture),
        |_, _report, _paths, _options, _failures, _notes| {},
    )
}

fn write_status_line(report: &mut String, fixture: &CompareFixtureReportInput<'_>) {
    let status = if fixture.failed { "FAIL" } else { "PASS" };
    let _ = writeln!(
        report,
        "- {status} `{}`\n  - fixture: `{}`\n  - upstream: `{}`\n  - local: `{}`",
        fixture.stem,
        fixture.fixture_path.display(),
        fixture.upstream_path.display(),
        fixture.local_out_path.display()
    );
}
