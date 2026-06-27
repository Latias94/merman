//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::cmd::compare::{
    CompareFixtureResult, CompareRunOptions, DEFAULT_ROOT_DELTA_REPORT_LIMIT, RootDelta,
    RootDeltaReportLimit, collect_root_delta, parse_root_delta_report_limit, run_svg_compare,
    sanitize_svg_id, svg_compare_engine_with_site_config, svg_compare_layout_opts,
    write_compare_result_section, write_notes_section, write_root_deltas_report,
};
use std::fmt::Write as _;
use std::path::PathBuf;

pub(crate) fn compare_class_svgs(args: Vec<String>) -> Result<(), XtaskError> {
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

    let should_report_root =
        report_root || matches!(dom_mode.trim(), "parity-root" | "parity_root");
    let engine = svg_compare_engine_with_site_config(serde_json::json!({ "handDrawnSeed": 1 }));
    let layout_opts = svg_compare_layout_opts();
    let mut state = ClassCompareState {
        root_deltas: Vec::new(),
    };

    run_svg_compare(
        CompareRunOptions {
            diagram: "class",
            out_path,
            filter: filter.as_deref(),
            check_dom,
            dom_mode: &dom_mode,
            dom_decimals,
        },
        &mut state,
        |_, report, _paths, options| {
            let _ = writeln!(
                report,
                "# ClassDiagram SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/class/*.svg` (pinned Mermaid baseline)\n- Local: `render_class_diagram_v2_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
                options.dom_mode, options.dom_decimals,
            );
        },
        |_, stem, _| {
            crate::cmd::upstream_svg_compare_skip_reason("class", stem)
                .map(|reason| reason.to_string())
        },
        |state, input| {
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

            let merman_render::model::LayoutDiagram::ClassDiagramV2(layout) = &layouted.layout
            else {
                return Err(format!(
                    "unexpected layout type for {}: {}",
                    input.fixture_path.display(),
                    layouted.meta.diagram_type
                ));
            };

            let is_classdiagram_v2_header =
                merman::preprocess_diagram(input.text, engine.registry())
                    .ok()
                    .map(|p| p.code.trim_start().starts_with("classDiagram-v2"))
                    .unwrap_or(false);

            let svg_opts = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(sanitize_svg_id(input.stem)),
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
                    return Err(format!(
                        "render failed for {}: {err}",
                        input.fixture_path.display()
                    ));
                }
            };

            if should_report_root {
                match collect_root_delta(input.stem, input.upstream_svg, &local_svg) {
                    Ok(delta) => state.root_deltas.push(delta),
                    Err(e) => {
                        return Ok(CompareFixtureResult::Rendered {
                            local_svg,
                            compare_dom: true,
                            issues: vec![format!("root parse failed for {}: {e}", input.stem)],
                            notes: Vec::new(),
                        });
                    }
                }
            }

            Ok(CompareFixtureResult::Rendered {
                local_svg,
                compare_dom: true,
                issues: Vec::new(),
                notes: Vec::new(),
            })
        },
        |state, report, paths, options, failures, notes| {
            if should_report_root {
                write_root_deltas_report(report, &mut state.root_deltas[..], root_report_limit);
            }
            write_compare_result_section(report, options.check_dom, failures, &paths.out_svg_dir);
            write_notes_section(report, notes);
        },
    )
}

struct ClassCompareState {
    root_deltas: Vec<RootDelta>,
}
