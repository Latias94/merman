//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::cmd::compare::{CompareFixtureResult, CompareRunOptions, run_svg_compare};
use crate::svgdom;
use regex::Regex;
use std::fmt::Write as _;
use std::path::PathBuf;

use super::super::{svg_compare_engine_with_site_config, svg_compare_layout_opts};

pub(crate) fn compare_er_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_markers: bool = false;
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
            "--check-markers" => check_markers = true,
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

    let engine = svg_compare_engine_with_site_config(serde_json::json!({ "handDrawnSeed": 1 }));
    let layout_opts = svg_compare_layout_opts();
    let re_marker_id = Regex::new(r#"<marker[^>]*\bid="([^"]+)""#).unwrap();
    let re_marker_ref = Regex::new(r#"marker-(?:start|end)="url\(#([^)]+)\)""#).unwrap();
    let mut state = ErCompareState {
        rows: Vec::new(),
        dom_failures: Vec::new(),
    };

    run_svg_compare(
        CompareRunOptions {
            diagram: "er",
            out_path,
            filter: filter.as_deref(),
            check_dom,
            dom_mode: &dom_mode,
            dom_decimals,
        },
        &mut state,
        |_, report, _paths, _options| {
            let _ = writeln!(report, "# ER SVG Compare Report");
            let _ = writeln!(report);
            let _ = writeln!(
                report,
                "- Upstream: `fixtures/upstream-svgs/er/*.svg` (pinned Mermaid baseline via Mermaid CLI)"
            );
            let _ = writeln!(report, "- Local: `render_er_diagram_svg` (Stage B)");
            let _ = writeln!(report);
            let _ = writeln!(
                report,
                "| fixture | markers ok | dom ok | viewBox (upstream) | viewBox (local) | max-width (upstream) | max-width (local) |"
            );
            let _ = writeln!(report, "|---|---:|---:|---|---|---:|---:|");
            let _ = writeln!(report);
        },
        |_, _, _| None,
        |state, input| {
            #[derive(Default)]
            struct SvgSig {
                marker_ids: std::collections::BTreeSet<String>,
                marker_refs: std::collections::BTreeSet<String>,
            }

            fn sig_for_svg(svg: &str, re_marker_id: &Regex, re_marker_ref: &Regex) -> SvgSig {
                let mut marker_ids = std::collections::BTreeSet::new();
                for cap in re_marker_id.captures_iter(svg) {
                    if let Some(m) = cap.get(1) {
                        marker_ids.insert(m.as_str().to_string());
                    }
                }
                let mut marker_refs = std::collections::BTreeSet::new();
                for cap in re_marker_ref.captures_iter(svg) {
                    if let Some(m) = cap.get(1) {
                        marker_refs.insert(m.as_str().to_string());
                    }
                }
                SvgSig {
                    marker_ids,
                    marker_refs,
                }
            }

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

            let merman_render::model::LayoutDiagram::ErDiagram(layout) = &layouted.layout else {
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

            let local_svg = match merman_render::svg::render_er_diagram_svg(
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

            let upstream_sig = sig_for_svg(input.upstream_svg, &re_marker_id, &re_marker_ref);
            let local_sig = sig_for_svg(&local_svg, &re_marker_id, &re_marker_ref);

            let mut issues: Vec<String> = Vec::new();
            let mut marker_ok = true;
            let mut missing: Vec<String> = Vec::new();
            let mut extra: Vec<String> = Vec::new();
            for m in &upstream_sig.marker_ids {
                if !local_sig.marker_ids.contains(m) {
                    marker_ok = false;
                    missing.push(m.clone());
                }
            }
            for m in &local_sig.marker_ids {
                if !upstream_sig.marker_ids.contains(m) {
                    marker_ok = false;
                    extra.push(m.clone());
                }
            }
            for r in &local_sig.marker_refs {
                if !local_sig.marker_ids.contains(r) {
                    marker_ok = false;
                    extra.push(format!("ref-missing-def:{r}"));
                }
            }

            let mut dom_ok = None;
            if input.check_dom {
                let upstream_dom =
                    match svgdom::dom_signature(input.upstream_svg, input.mode, input.dom_decimals)
                    {
                        Ok(v) => Some(v),
                        Err(err) => {
                            issues.push(format!(
                                "dom parse failed (upstream) for {}: {err}",
                                input.stem
                            ));
                            None
                        }
                    };
                let local_dom =
                    match svgdom::dom_signature(&local_svg, input.mode, input.dom_decimals) {
                        Ok(v) => Some(v),
                        Err(err) => {
                            issues.push(format!(
                                "dom parse failed (local) for {}: {err}",
                                input.stem
                            ));
                            None
                        }
                    };

                if let (Some(upstream_dom), Some(local_dom)) = (upstream_dom, local_dom) {
                    if let Some(diff) = svgdom::dom_diff(&upstream_dom, &local_dom) {
                        issues.push(format!(
                            "dom mismatch for {} (mode={}, decimals={})",
                            input.stem, dom_mode, dom_decimals
                        ));
                        state.dom_failures.push(format!("{}: {diff}", input.stem));
                        dom_ok = Some(false);
                    } else {
                        dom_ok = Some(true);
                    }
                } else {
                    dom_ok = Some(false);
                }
            }

            if check_markers && !marker_ok {
                issues.push(format!(
                    "marker mismatch for {}: missing={:?} extra={:?}",
                    input.stem, missing, extra
                ));
            }

            state.rows.push(ErCompareRow {
                stem: input.stem.to_string(),
                marker_ok,
                dom_ok,
                upstream_view_box: extract_view_box(input.upstream_svg),
                local_view_box: extract_view_box(&local_svg),
                upstream_max_width: extract_max_width(input.upstream_svg),
                local_max_width: extract_max_width(&local_svg),
            });

            Ok(CompareFixtureResult::Rendered {
                local_svg,
                compare_dom: false,
                issues,
                notes: Vec::new(),
            })
        },
        |state, report, _paths, options, _failures, _notes| {
            for row in &state.rows {
                let dom_ok = match row.dom_ok {
                    Some(true) => "yes",
                    Some(false) => "no",
                    None => "-",
                };
                let _ = writeln!(
                    report,
                    "| `{}` | {} | {} | `{}` | `{}` | `{}` | `{}` |",
                    row.stem,
                    if row.marker_ok { "yes" } else { "no" },
                    dom_ok,
                    row.upstream_view_box,
                    row.local_view_box,
                    row.upstream_max_width,
                    row.local_max_width,
                );
            }

            if options.check_dom && !state.dom_failures.is_empty() {
                let _ = writeln!(report);
                let _ = writeln!(report, "## DOM Mismatch Details");
                let _ = writeln!(report);
                for detail in &state.dom_failures {
                    let _ = writeln!(report, "- {detail}");
                }
            }
        },
    )
}

struct ErCompareState {
    rows: Vec<ErCompareRow>,
    dom_failures: Vec<String>,
}

struct ErCompareRow {
    stem: String,
    marker_ok: bool,
    dom_ok: Option<bool>,
    upstream_view_box: String,
    local_view_box: String,
    upstream_max_width: String,
    local_max_width: String,
}

fn extract_view_box(svg: &str) -> String {
    Regex::new(r#"viewBox="([^"]+)""#)
        .unwrap()
        .captures(svg)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn extract_max_width(svg: &str) -> String {
    Regex::new(r#"max-width:\s*([0-9.]+)px"#)
        .unwrap()
        .captures(svg)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_else(|| "-".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_svg_root_signatures() {
        let svg = r#"<svg viewBox="0 0 10 20" style="max-width: 10px;"><g /></svg>"#;
        assert_eq!(extract_view_box(svg), "0 0 10 20");
        assert_eq!(extract_max_width(svg), "10");
    }
}
