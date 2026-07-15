//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::cmd::compare::{
    CompareFixtureResult, CompareHarnessOptions, CompareRequest, CompareRunOptions,
    DiagramVerificationFact, ObservedRenderOperations, compare_render_environment, run_svg_compare,
    write_compare_result_section, write_verification_policy_metadata,
};
use regex::Regex;
use std::fmt::Write as _;
use std::path::PathBuf;

use super::super::{svg_compare_engine_with_site_config, svg_compare_layout_opts};

pub(super) fn compare_er_args(
    fact: DiagramVerificationFact,
    args: Vec<String>,
) -> Result<(), XtaskError> {
    let mut request = CompareRequest::default();
    let mut check_markers = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                request.out_path = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                request.filter = args.get(i).cloned();
            }
            "--check-markers" => check_markers = true,
            "--check-dom" => request.check_dom = true,
            "--no-root-overrides" => request.apply_root_overrides = false,
            "--dom-decimals" => {
                i += 1;
                request.dom_decimals = Some(
                    args.get(i)
                        .and_then(|value| value.parse::<u32>().ok())
                        .unwrap_or(3),
                );
            }
            "--dom-mode" => {
                i += 1;
                request.dom_mode = Some(
                    args.get(i)
                        .map(|s| s.trim().to_string())
                        .unwrap_or_else(|| fact.default_dom_mode.to_string()),
                );
            }
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    run_er_compare(
        fact,
        ErCompareRequest {
            common: request,
            check_markers,
        },
    )
}

pub(super) fn compare_er_request(
    fact: DiagramVerificationFact,
    request: CompareRequest,
) -> Result<(), XtaskError> {
    run_er_compare(
        fact,
        ErCompareRequest {
            common: request,
            check_markers: false,
        },
    )
}

struct ErCompareRequest {
    common: CompareRequest,
    check_markers: bool,
}

fn run_er_compare(
    fact: DiagramVerificationFact,
    request: ErCompareRequest,
) -> Result<(), XtaskError> {
    let dom_mode = request
        .common
        .dom_mode
        .as_deref()
        .unwrap_or(fact.default_dom_mode);
    let dom_decimals = request.common.dom_decimals.unwrap_or(3);

    let engine = svg_compare_engine_with_site_config(serde_json::json!({ "handDrawnSeed": 1 }));
    let layout_opts = svg_compare_layout_opts();
    let environment = compare_render_environment(&request.common);
    let observed_operations = ObservedRenderOperations::from_environment(&environment)?;
    let renderer = merman::render::HeadlessRenderer::new()
        .with_engine(engine)
        .with_parse_options(fact.parse_policy.options())
        .with_layout_options(layout_opts)
        .with_environment(environment);
    let re_marker_id = Regex::new(r#"<marker[^>]*\bid="([^"]+)""#).unwrap();
    let re_marker_ref = Regex::new(r#"marker-(?:start|end)="url\(#([^)]+)\)""#).unwrap();
    let mut state = ErCompareState {
        rows: Vec::new(),
        observed_operations,
    };

    run_svg_compare(
        CompareHarnessOptions::new(CompareRunOptions {
            diagram: fact.diagram,
            out_path: request.common.out_path.clone(),
            filter: request.common.filter.as_deref(),
            check_dom: request.common.check_dom,
            dom_mode,
            dom_decimals,
        }),
        &mut state,
        |_, report, _paths, options| {
            let _ = writeln!(report, "# {} SVG Compare Report", fact.report_title);
            let _ = writeln!(report);
            let _ = writeln!(
                report,
                "- Upstream: `fixtures/upstream-svgs/er/*.svg` (pinned Mermaid baseline via Mermaid CLI)"
            );
            let _ = writeln!(report, "- Command: `{}`", fact.command);
            let _ = writeln!(report, "- Mode: `{}`", options.dom_mode);
            let _ = writeln!(report, "- Decimals: `{}`", options.dom_decimals);
            write_verification_policy_metadata(
                report,
                &request.common,
                fact,
                options.dom_mode,
                false,
            );
            let _ = writeln!(report);
            let _ = writeln!(
                report,
                "| fixture | markers ok | viewBox (upstream) | viewBox (local) | max-width (upstream) | max-width (local) |"
            );
            let _ = writeln!(report, "|---|---:|---|---|---:|---:|");
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

            let semantic = match renderer.prepare_semantic_sync(input.text) {
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

            if prepared.family_kind() != merman::render::RenderFamilyKind::Er {
                return Err(format!(
                    "unexpected render family for {}: {}",
                    input.fixture_path.display(),
                    prepared.family_kind()
                ));
            }

            let svg_opts = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(input.stem.to_string()),
                ..Default::default()
            };

            let rendered = match prepared.render_svg_report(&svg_opts) {
                Ok(v) => v,
                Err(err) => {
                    return Err(format!(
                        "render failed for {}: {err}",
                        input.fixture_path.display()
                    ));
                }
            };
            state
                .observed_operations
                .observe(input.stem, rendered.report())?;
            let local_svg = rendered.into_svg();

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

            if request.check_markers && !marker_ok {
                issues.push(format!(
                    "marker mismatch for {}: missing={:?} extra={:?}",
                    input.stem, missing, extra
                ));
            }

            state.rows.push(ErCompareRow {
                stem: input.stem.to_string(),
                marker_ok,
                upstream_view_box: extract_view_box(input.upstream_svg),
                local_view_box: extract_view_box(&local_svg),
                upstream_max_width: extract_max_width(input.upstream_svg),
                local_max_width: extract_max_width(&local_svg),
            });

            Ok(CompareFixtureResult::Rendered {
                local_svg,
                compare_dom: true,
                issues,
                notes: Vec::new(),
            })
        },
        |_, _, _| {},
        |state, report, paths, options, failures, _notes| {
            state.observed_operations.write_report(report);
            for row in &state.rows {
                let _ = writeln!(
                    report,
                    "| `{}` | {} | `{}` | `{}` | `{}` | `{}` |",
                    row.stem,
                    if row.marker_ok { "yes" } else { "no" },
                    row.upstream_view_box,
                    row.local_view_box,
                    row.upstream_max_width,
                    row.local_max_width,
                );
            }
            write_compare_result_section(report, options.check_dom, failures, &paths.out_svg_dir);
        },
    )
}

struct ErCompareState {
    rows: Vec<ErCompareRow>,
    observed_operations: ObservedRenderOperations,
}

struct ErCompareRow {
    stem: String,
    marker_ok: bool,
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
