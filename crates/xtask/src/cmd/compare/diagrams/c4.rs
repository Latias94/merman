//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::cmd::compare::{
    CompareFixtureResult, CompareRunOptions, run_svg_compare, sanitize_svg_id, svg_compare_engine,
    svg_compare_layout_opts, write_compare_result_section, write_notes_section,
};
use std::fmt::Write as _;
use std::path::PathBuf;

fn c4_fixture_is_excluded(stem: &str) -> bool {
    matches!(
        stem,
        "nesting_updates"
            | "upstream_boundary_spec"
            | "upstream_c4container_header_and_direction_spec"
            | "upstream_container_spec"
            | "upstream_person_ext_spec"
            | "upstream_person_spec"
            | "upstream_system_spec"
            | "upstream_update_element_style_all_fields_spec"
    )
}

pub(crate) fn compare_c4_svgs(args: Vec<String>) -> Result<(), XtaskError> {
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
                    .unwrap_or_else(|| "structure".to_string());
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
            diagram: "c4",
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
                "# C4 SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/c4/*.svg` (pinned Mermaid baseline)\n- Local: `render_c4_diagram_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n",
                options.dom_mode, options.dom_decimals
            );
        },
        |_, stem, _| {
            if c4_fixture_is_excluded(stem) {
                Some("deferred upstream baseline / feature-gated parity case".to_string())
            } else {
                None
            }
        },
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

            let merman_render::model::LayoutDiagram::C4Diagram(layout) = &layouted.layout else {
                return Err(format!(
                    "unexpected layout type for {}: {}",
                    input.fixture_path.display(),
                    layouted.meta.diagram_type
                ));
            };

            let svg_opts = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(sanitize_svg_id(input.stem)),
                ..Default::default()
            };

            let local_svg = match merman_render::svg::render_c4_diagram_svg(
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
