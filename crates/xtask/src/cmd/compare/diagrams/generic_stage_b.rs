//! Generic Stage B SVG compare command for small typed-render diagrams.

use crate::XtaskError;
use crate::cmd::compare::{
    CompareFixtureResult, CompareRunOptions, run_svg_compare, sanitize_svg_id,
    write_compare_result_section, write_notes_section,
};
use std::fmt::Write as _;
use std::path::PathBuf;

use super::super::{svg_compare_engine, svg_compare_layout_opts};

struct StageBCompareSpec {
    diagram_dir: &'static str,
    report_title: &'static str,
    local_renderer: &'static str,
}

pub(crate) fn compare_tree_view_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    compare_stage_b_svgs(
        args,
        StageBCompareSpec {
            diagram_dir: "treeView",
            report_title: "TreeView",
            local_renderer: "render_layouted_svg treeView dispatch (Stage B)",
        },
    )
}

pub(crate) fn compare_ishikawa_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    compare_stage_b_svgs(
        args,
        StageBCompareSpec {
            diagram_dir: "ishikawa",
            report_title: "Ishikawa",
            local_renderer: "render_layouted_svg ishikawa dispatch (Stage B)",
        },
    )
}

pub(crate) fn compare_eventmodeling_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    compare_stage_b_svgs(
        args,
        StageBCompareSpec {
            diagram_dir: "eventmodeling",
            report_title: "EventModeling",
            local_renderer: "render_layouted_svg eventmodeling dispatch (Stage B)",
        },
    )
}

pub(crate) fn compare_venn_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    compare_stage_b_svgs(
        args,
        StageBCompareSpec {
            diagram_dir: "venn",
            report_title: "Venn",
            local_renderer: "render_layouted_svg venn dispatch (Stage B)",
        },
    )
}

pub(crate) fn compare_cynefin_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    compare_stage_b_svgs(
        args,
        StageBCompareSpec {
            diagram_dir: "cynefin",
            report_title: "Cynefin",
            local_renderer: "render_layouted_svg cynefin dispatch (Stage B)",
        },
    )
}

pub(crate) fn compare_railroad_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    compare_stage_b_svgs(
        args,
        StageBCompareSpec {
            diagram_dir: "railroad",
            report_title: "Railroad",
            local_renderer: "render_layouted_svg railroad dispatch (Stage B)",
        },
    )
}

pub(crate) fn compare_railroad_ebnf_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    compare_stage_b_svgs(
        args,
        StageBCompareSpec {
            diagram_dir: "railroadEbnf",
            report_title: "Railroad EBNF",
            local_renderer: "render_layouted_svg railroad EBNF dispatch (Stage B)",
        },
    )
}

pub(crate) fn compare_railroad_abnf_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    compare_stage_b_svgs(
        args,
        StageBCompareSpec {
            diagram_dir: "railroadAbnf",
            report_title: "Railroad ABNF",
            local_renderer: "render_layouted_svg railroad ABNF dispatch (Stage B)",
        },
    )
}

pub(crate) fn compare_railroad_peg_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    compare_stage_b_svgs(
        args,
        StageBCompareSpec {
            diagram_dir: "railroadPeg",
            report_title: "Railroad PEG",
            local_renderer: "render_layouted_svg railroad PEG dispatch (Stage B)",
        },
    )
}

fn compare_stage_b_svgs(args: Vec<String>, spec: StageBCompareSpec) -> Result<(), XtaskError> {
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

    let engine = svg_compare_engine();
    let layout_opts = svg_compare_layout_opts();

    run_svg_compare(
        CompareRunOptions {
            diagram: spec.diagram_dir,
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
                "# {} SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/{}/*.svg` (pinned Mermaid baseline)\n- Local: `{}`\n- Mode: `{}`\n- Decimals: `{}`\n",
                spec.report_title,
                spec.diagram_dir,
                spec.local_renderer,
                options.dom_mode,
                options.dom_decimals
            );
        },
        |_, _, _| None,
        |_, input| {
            let parsed = match futures::executor::block_on(engine.parse_diagram(
                input.text,
                merman::ParseOptions {
                    suppress_errors: true,
                },
            )) {
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

            let svg_opts = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(sanitize_svg_id(input.stem)),
                ..Default::default()
            };

            let local_svg = match merman_render::svg::render_layouted_svg(
                &layouted,
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

            Ok(CompareFixtureResult::Rendered {
                local_svg,
                compare_dom: true,
                issues: Vec::new(),
                notes: Vec::new(),
            })
        },
        |_, report, paths, options, failures, notes| {
            write_compare_result_section(report, options.check_dom, failures, &paths.out_svg_dir);
            write_notes_section(report, notes);
        },
    )
}
