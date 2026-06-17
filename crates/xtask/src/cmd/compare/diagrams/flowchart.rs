//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::cmd::compare::{
    DEFAULT_LABEL_DELTA_REPORT_LIMIT, DEFAULT_ROOT_DELTA_REPORT_LIMIT, LabelDeltaReportLimit,
    LabelMetricDelta, RootDelta, RootDeltaReportLimit, collect_label_metric_deltas,
    parse_label_delta_report_limit, parse_root_attrs, parse_root_delta_report_limit,
    write_label_deltas_report, write_root_deltas_report,
};
use crate::svgdom;
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

fn resolve_compare_root(raw: Option<PathBuf>, default: PathBuf) -> PathBuf {
    let Some(raw) = raw else {
        return default;
    };
    if raw.is_absolute() {
        return raw;
    }
    if raw.as_os_str().is_empty() {
        return default;
    }
    crate::cmd::workspace_root().join(raw)
}

pub(crate) fn compare_flowchart_svgs(args: Vec<String>) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut fixtures_root_arg: Option<PathBuf> = None;
    let mut upstream_root_arg: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut report_root: bool = false;
    let mut root_report_limit = DEFAULT_ROOT_DELTA_REPORT_LIMIT;
    let mut report_root_pins_only: bool = false;
    let mut report_label: bool = false;
    let mut label_report_limit = DEFAULT_LABEL_DELTA_REPORT_LIMIT;
    let mut report_label_root_pins_only: bool = false;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode: String = "parity".to_string();
    let mut text_measurer: String = "vendored".to_string();
    let mut apply_root_overrides: bool = true;
    let mut include_elk_probes: bool = false;
    let mut force_elk_fixture: bool = false;
    let mut flowchart_elk_backend = merman_render::FlowchartElkBackend::Compat;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                out_path = args.get(i).map(PathBuf::from);
            }
            "--fixtures-root" => {
                i += 1;
                fixtures_root_arg = args.get(i).map(PathBuf::from);
            }
            "--upstream-root" => {
                i += 1;
                upstream_root_arg = args.get(i).map(PathBuf::from);
            }
            "--filter" => {
                i += 1;
                filter = args.get(i).map(|s| s.to_string());
            }
            "--check-dom" => check_dom = true,
            "--report-root" => report_root = true,
            "--report-root-pins-only" => {
                report_root = true;
                report_root_pins_only = true;
            }
            "--report-root-all" => {
                report_root = true;
                root_report_limit = RootDeltaReportLimit::All;
            }
            "--report-label" => report_label = true,
            "--report-label-root-pins-only" => {
                report_label = true;
                report_label_root_pins_only = true;
            }
            "--report-label-all" => {
                report_label = true;
                label_report_limit = LabelDeltaReportLimit::All;
            }
            "--report-root-limit" => {
                i += 1;
                report_root = true;
                root_report_limit = parse_root_delta_report_limit(args.get(i).map(String::as_str))?;
            }
            "--report-label-limit" => {
                i += 1;
                report_label = true;
                label_report_limit =
                    parse_label_delta_report_limit(args.get(i).map(String::as_str))?;
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
                    .unwrap_or_else(|| "structure".to_string());
            }
            "--text-measurer" => {
                i += 1;
                text_measurer = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "deterministic".to_string());
            }
            "--flowchart-elk-backend" => {
                i += 1;
                flowchart_elk_backend =
                    parse_flowchart_elk_backend(args.get(i).map(String::as_str))?;
            }
            "--no-root-overrides" => apply_root_overrides = false,
            "--include-elk-probes" => include_elk_probes = true,
            "--force-elk-fixture" => force_elk_fixture = true,
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    if force_elk_fixture
        && flowchart_elk_backend != merman_render::FlowchartElkBackend::SourcePorted
    {
        return Err(XtaskError::SvgCompareFailed(
            "`--force-elk-fixture` requires `--flowchart-elk-backend source-ported`".to_string(),
        ));
    }

    let compare_paths = crate::cmd::compare_diagram_paths("flowchart", out_path);
    let fixtures_dir = resolve_compare_root(fixtures_root_arg, compare_paths.fixtures_dir);
    let upstream_dir = resolve_compare_root(upstream_root_arg, compare_paths.upstream_dir);
    let out_path = compare_paths.out_path;
    let out_svg_dir = compare_paths.out_svg_dir;
    let mmd_files = crate::cmd::list_mmd_fixtures_in_dir(&fixtures_dir, filter.as_deref(), true);

    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(&dom_mode);
    let should_report_root = report_root || mode == svgdom::DomMode::ParityRoot;

    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(
        serde_json::json!({ "handDrawnSeed": 1 }),
    ));
    let mut layout_opts = merman_render::LayoutOptions::default();
    if matches!(
        text_measurer.as_str(),
        "vendored" | "vendored-font" | "vendored-font-metrics"
    ) {
        layout_opts.text_measurer =
            std::sync::Arc::new(merman_render::text::VendoredFontMetricsTextMeasurer::default());
    }
    let flowchart_math_renderer: Option<Arc<dyn merman_render::math::MathRenderer + Send + Sync>> = {
        let node_cwd = crate::cmd::mermaid_cli_root();
        if node_cwd.join("package.json").is_file() && node_cwd.join("node_modules").is_dir() {
            Some(Arc::new(merman_render::math::NodeKatexMathRenderer::new(
                node_cwd,
            )))
        } else {
            None
        }
    };
    if let Some(renderer) = flowchart_math_renderer.clone() {
        layout_opts.math_renderer = Some(renderer);
    }
    layout_opts.flowchart_elk_backend = flowchart_elk_backend;
    let mut report = String::new();
    let _ = writeln!(
        &mut report,
        "# Flowchart SVG Comparison\n\n- Upstream: `fixtures/upstream-svgs/flowchart/*.svg` (pinned Mermaid baseline)\n- Local: `render_flowchart_v2_svg` (Stage B)\n- Mode: `{}`\n- Decimals: `{}`\n- Text measurer: `{}`\n- Math renderer: `{}`\n- Root overrides: `{}`\n- Flowchart ELK backend: `{}`\n- Forced ELK fixtures: `{}`\n- Root rows: `{}`\n- Label rows: `{}`\n",
        dom_mode,
        dom_decimals,
        text_measurer,
        if flowchart_math_renderer.is_some() {
            "node-katex"
        } else {
            "none"
        },
        if apply_root_overrides {
            "enabled"
        } else {
            "disabled"
        },
        flowchart_elk_backend_name(flowchart_elk_backend),
        if force_elk_fixture {
            "enabled"
        } else {
            "disabled"
        },
        if report_root_pins_only {
            "root-pins-only"
        } else {
            "all fixtures"
        },
        if report_label_root_pins_only {
            "root-pins-only"
        } else {
            "all fixtures"
        }
    );

    let mut root_deltas: Vec<RootDelta> = Vec::new();
    let mut label_deltas: Vec<LabelMetricDelta> = Vec::new();
    let flowchart_root_pin_ids =
        if report_label || report_label_root_pins_only || report_root_pins_only {
            collect_flowchart_root_pin_ids()
        } else {
            std::collections::BTreeSet::new()
        };

    let mut failures: Vec<String> = Vec::new();
    let mut skipped: Vec<String> = Vec::new();
    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        if let Some(reason) = crate::cmd::upstream_svg_baseline_skip_reason("flowchart", stem) {
            skipped.push(format!("skipped {stem}: {reason}"));
            continue;
        }

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg {}: {err}",
                    upstream_path.display()
                ));
                continue;
            }
        };
        let diagram_id: String = if stem.ends_with("_katex") {
            roxmltree::Document::parse(&upstream_svg)
                .ok()
                .and_then(|doc| doc.root_element().attribute("id").map(str::to_string))
                .filter(|id| !id.trim().is_empty())
                .unwrap_or_else(|| {
                    stem.chars()
                        .map(|ch| {
                            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                                ch
                            } else {
                                '_'
                            }
                        })
                        .collect()
                })
        } else {
            stem.chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                        ch
                    } else {
                        '_'
                    }
                })
                .collect()
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        // Upstream Mermaid renders `$$...$$` fragments via KaTeX (JS) and measures the resulting
        // HTML via DOM. When the local Node/Puppeteer-backed math backend is unavailable, keep
        // these fixtures renderable but skip strict DOM assertions.
        let skip_dom_compare_for_math =
            check_dom && text.contains("$$") && flowchart_math_renderer.is_none();

        let parsed = match futures::executor::block_on(
            engine.parse_diagram(&text, merman::ParseOptions::default()),
        ) {
            Ok(Some(v)) => v,
            Ok(None) => {
                failures.push(format!("no diagram detected in {}", mmd_path.display()));
                continue;
            }
            Err(err) => {
                failures.push(format!("parse failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let flowchart_layout_elk = parsed.meta.effective_config.get_str("layout") == Some("elk")
            || parsed
                .meta
                .effective_config
                .get_str("flowchart.defaultRenderer")
                == Some("elk");
        if parsed.meta.diagram_type == "flowchart-elk" || flowchart_layout_elk {
            let admitted = crate::cmd::flowchart_elk_svg_compare_admitted(
                stem,
                include_elk_probes,
                flowchart_elk_backend,
            );
            if !admitted
                && !force_elk_fixture
                && let Some(reason) = crate::cmd::flowchart_elk_svg_parity_skip_reason(stem)
            {
                skipped.push(format!("skipped {stem}: {reason}"));
                continue;
            }
        }

        let layouted = match merman_render::layout_parsed(&parsed, &layout_opts) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("layout failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let merman_render::model::LayoutDiagram::FlowchartV2(layout) = &layouted.layout else {
            failures.push(format!(
                "unexpected layout type for {}: {}",
                mmd_path.display(),
                layouted.meta.diagram_type
            ));
            continue;
        };

        let svg_opts = merman_render::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            aria_roledescription: Some(parsed.meta.diagram_type.clone()),
            math_renderer: flowchart_math_renderer.clone(),
            apply_root_overrides,
            ..Default::default()
        };

        let local_svg = match merman_render::svg::render_flowchart_v2_svg(
            layout,
            &layouted.semantic,
            &layouted.meta.effective_config,
            layouted.meta.title.as_deref(),
            layout_opts.text_measurer.as_ref(),
            &svg_opts,
        ) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("render failed for {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let _ = fs::write(&local_out_path, &local_svg);

        let root_pinned = flowchart_root_pin_ids.contains(stem);
        if report_label && (!report_label_root_pins_only || root_pinned) {
            match collect_label_metric_deltas(stem, &upstream_svg, &local_svg, root_pinned) {
                Ok(mut rows) => label_deltas.append(&mut rows),
                Err(e) => failures.push(format!("label metric parse failed for {stem}: {e}")),
            }
        }

        if should_report_root && (!report_root_pins_only || root_pinned) {
            match (
                parse_root_attrs(&upstream_svg),
                parse_root_attrs(&local_svg),
            ) {
                (Ok(up), Ok(lo)) => root_deltas.push(RootDelta {
                    stem: stem.to_string(),
                    max_width_delta: match (up.max_width_px, lo.max_width_px) {
                        (Some(a), Some(b)) => Some(b - a),
                        _ => None,
                    },
                    upstream: up,
                    local: lo,
                }),
                (Err(e), _) => failures.push(format!("root parse failed for upstream {stem}: {e}")),
                (_, Err(e)) => failures.push(format!("root parse failed for local {stem}: {e}")),
            }
        }

        if check_dom && !skip_dom_compare_for_math {
            let a = match svgdom::dom_signature(&upstream_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("upstream dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            let b = match svgdom::dom_signature(&local_svg, mode, dom_decimals) {
                Ok(v) => v,
                Err(err) => {
                    failures.push(format!("local dom parse failed for {stem}: {err}"));
                    continue;
                }
            };
            if a != b {
                let detail = svgdom::dom_diff(&a, &b)
                    .map(|d| format!(" ({d})"))
                    .unwrap_or_default();
                failures.push(format!(
                    "dom mismatch for {stem}: upstream={} local={}{}",
                    upstream_path.display(),
                    local_out_path.display(),
                    detail
                ));
            }
        } else if check_dom && skip_dom_compare_for_math {
            skipped.push(format!(
                "skipped {stem}: contains `$$...$$` but no local Node/Puppeteer KaTeX backend was available"
            ));
        }
    }

    if !check_dom {
        let _ = writeln!(
            &mut report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(&mut report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(&mut report, "\n## Mismatches\n");
        for f in &failures {
            let _ = writeln!(&mut report, "- {f}");
        }
        let _ = writeln!(
            &mut report,
            "\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    }

    if !skipped.is_empty() {
        let _ = writeln!(
            &mut report,
            "\n## Skipped\n\nThese fixtures are intentionally skipped (feature gaps / deferred parity).\n"
        );
        for s in &skipped {
            let _ = writeln!(&mut report, "- {s}");
        }
    }

    if should_report_root {
        write_root_deltas_report(&mut report, &mut root_deltas[..], root_report_limit);
    }
    if report_label {
        write_label_deltas_report(&mut report, &mut label_deltas[..], label_report_limit);
    }

    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&out_path, report).map_err(|source| XtaskError::WriteFile {
        path: out_path.display().to_string(),
        source,
    })?;

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

pub(crate) fn check_flowchart_elk_source_backed_probes(
    args: Vec<String>,
) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }

    let mut failures = Vec::new();
    for stem in crate::cmd::flowchart_elk_svg_source_backed_probe_stems() {
        let out_path = crate::cmd::target_root()
            .join("compare")
            .join("flowchart_elk_source_backed")
            .join(format!("{stem}.md"));
        let result = compare_flowchart_svgs(vec![
            "--filter".to_string(),
            (*stem).to_string(),
            "--include-elk-probes".to_string(),
            "--flowchart-elk-backend".to_string(),
            "source-ported".to_string(),
            "--check-dom".to_string(),
            "--dom-mode".to_string(),
            "parity".to_string(),
            "--dom-decimals".to_string(),
            "3".to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
        ]);
        if let Err(err) = result {
            failures.push(format!("{stem}: {err}"));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

pub(crate) fn audit_flowchart_elk_source_backed_coverage(
    args: Vec<String>,
) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }

    let spec_path = crate::cmd::mermaid_repo_root()
        .join("cypress")
        .join("integration")
        .join("rendering")
        .join("flowchart")
        .join("flowchart-elk.spec.js");
    let spec = fs::read_to_string(&spec_path).map_err(|source| XtaskError::ReadFile {
        path: spec_path.display().to_string(),
        source,
    })?;

    let fixture_dir = crate::cmd::fixtures_root().join("flowchart");
    let upstream_svg_dir = crate::cmd::fixtures_root()
        .join("upstream-svgs")
        .join("flowchart");

    let cases = collect_flowchart_elk_spec_snapshot_cases(&spec)?;
    let admitted = crate::cmd::flowchart_elk_svg_source_backed_probe_stems();
    let mut admitted_layout_body_keys: BTreeMap<String, String> = BTreeMap::new();
    for stem in admitted {
        let fixture_path = fixture_dir.join(format!("{stem}.mmd"));
        if let Ok(text) = fs::read_to_string(&fixture_path) {
            admitted_layout_body_keys
                .entry(canonical_flowchart_elk_layout_body_key(&text))
                .or_insert_with(|| (*stem).to_string());
        }
    }

    for case in &cases {
        if admitted.contains(&case.stem.as_str()) {
            admitted_layout_body_keys
                .entry(case.layout_body_key.clone())
                .or_insert_with(|| case.stem.clone());
        }
    }

    let mut admitted_count = 0usize;
    let mut fixture_count = 0usize;
    let mut upstream_svg_count = 0usize;
    let mut missing = Vec::new();
    let mut not_admitted = Vec::new();
    let mut no_upstream_svg = Vec::new();
    let mut unique_layout_body_keys = BTreeSet::new();
    let mut covered_layout_body_keys = BTreeSet::new();
    let mut duplicate_covered = Vec::new();
    let mut uncovered_layout_body_groups: BTreeMap<String, Vec<&FlowchartElkSpecCase>> =
        BTreeMap::new();

    for case in &cases {
        let fixture_path = fixture_dir.join(format!("{}.mmd", case.stem));
        let svg_path = upstream_svg_dir.join(format!("{}.svg", case.stem));
        let has_fixture = fixture_path.is_file();
        let has_svg = svg_path.is_file();
        let is_admitted = admitted.contains(&case.stem.as_str());
        let covered_by_layout_body = admitted_layout_body_keys
            .get(&case.layout_body_key)
            .map(String::as_str);

        unique_layout_body_keys.insert(case.layout_body_key.clone());
        if covered_by_layout_body.is_some() {
            covered_layout_body_keys.insert(case.layout_body_key.clone());
        } else {
            uncovered_layout_body_groups
                .entry(case.layout_body_key.clone())
                .or_default()
                .push(case);
        }

        if has_fixture {
            fixture_count += 1;
        } else {
            missing.push(case);
        }
        if has_svg {
            upstream_svg_count += 1;
        } else {
            no_upstream_svg.push(case);
        }
        if is_admitted {
            admitted_count += 1;
        } else {
            not_admitted.push(case);
            if let Some(representative) = covered_by_layout_body {
                duplicate_covered.push((case, representative));
            }
        }
    }

    let uncovered_layout_body_count =
        unique_layout_body_keys.len() - covered_layout_body_keys.len();

    println!("Flowchart ELK source-backed coverage");
    println!("spec: {}", spec_path.display());
    println!("ELK render calls: {}", cases.len());
    println!("fixtures present: {fixture_count}");
    println!("upstream SVGs present: {upstream_svg_count}");
    println!("source-backed admitted: {admitted_count}");
    println!("missing fixtures: {}", missing.len());
    println!("missing upstream SVGs: {}", no_upstream_svg.len());
    println!("not admitted: {}", not_admitted.len());
    println!("unique layout bodies: {}", unique_layout_body_keys.len());
    println!(
        "unique layout bodies covered by admitted probes: {}",
        covered_layout_body_keys.len()
    );
    println!("uncovered unique layout bodies: {uncovered_layout_body_count}");
    println!(
        "not admitted but covered by duplicate layout body: {}",
        duplicate_covered.len()
    );

    if !duplicate_covered.is_empty() {
        println!();
        println!("Exact calls covered through an admitted duplicate layout body:");
        for (case, representative) in &duplicate_covered {
            println!(
                "- {} {} [{}{}]",
                case.case_number,
                case.test_name,
                case.call,
                if case.snapshot { ", snapshot" } else { "" }
            );
            println!("  stem: {}", case.stem);
            println!("  covered_by: {representative}");
        }
    }

    if !uncovered_layout_body_groups.is_empty() {
        println!();
        println!("Uncovered unique layout bodies:");
        let mut groups = uncovered_layout_body_groups.values().collect::<Vec<_>>();
        groups.sort_by_key(|group| group[0].case_number);
        for group in groups {
            let representative = group[0];
            println!(
                "- {} {} [{}{}]",
                representative.case_number,
                representative.test_name,
                representative.call,
                if representative.snapshot {
                    ", snapshot"
                } else {
                    ""
                }
            );
            println!("  stem: {}", representative.stem);
            if group.len() > 1 {
                let duplicates = group
                    .iter()
                    .skip(1)
                    .map(|case| case.stem.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                println!("  duplicate_calls: {duplicates}");
            }
        }
    }

    if !missing.is_empty() {
        println!();
        println!("Missing fixtures:");
        for case in &missing {
            println!(
                "- {} {} [{}{}]",
                case.case_number,
                case.test_name,
                case.call,
                if case.snapshot { ", snapshot" } else { "" }
            );
            println!("  stem: {}", case.stem);
        }
    }

    if !not_admitted.is_empty() {
        println!();
        println!("Not admitted:");
        for case in &not_admitted {
            println!(
                "- {} {} [{}{}]",
                case.case_number,
                case.test_name,
                case.call,
                if case.snapshot { ", snapshot" } else { "" }
            );
            println!("  stem: {}", case.stem);
        }
    }

    if !no_upstream_svg.is_empty() {
        println!();
        println!("Missing upstream SVGs:");
        for case in &no_upstream_svg {
            println!(
                "- {} {} [{}{}]",
                case.case_number,
                case.test_name,
                case.call,
                if case.snapshot { ", snapshot" } else { "" }
            );
            println!("  stem: {}", case.stem);
        }
    }

    Ok(())
}

#[derive(Debug)]
struct FlowchartElkSpecCase {
    case_number: usize,
    test_name: String,
    stem: String,
    layout_body_key: String,
    call: &'static str,
    snapshot: bool,
}

fn collect_flowchart_elk_spec_snapshot_cases(
    spec: &str,
) -> Result<Vec<FlowchartElkSpecCase>, XtaskError> {
    let source_slug = clamp_flowchart_elk_slug(slugify_flowchart_elk("flowchart-elk spec"), 48);
    let mut cases = Vec::new();
    let it_positions = collect_flowchart_elk_it_positions(spec);
    let bytes = spec.as_bytes();
    let mut idx_in_file = 0usize;

    for (call, needle) in [
        ("imgSnapshotTest", "imgSnapshotTest"),
        ("renderGraph", "renderGraph"),
    ] {
        let mut search_from = 0usize;
        while let Some(abs) = find_flowchart_elk_call(spec, needle, search_from) {
            let current_it = flowchart_elk_test_at(&it_positions, abs);
            let skipped_it = current_it.is_some_and(|it| it.skipped);
            if skipped_it {
                search_from = abs + needle.len();
                continue;
            }

            let after_call = abs + needle.len();
            let mut open_paren = after_call;
            while bytes
                .get(open_paren)
                .is_some_and(|b| is_flowchart_elk_ws_byte(*b))
            {
                open_paren += 1;
            }
            if bytes.get(open_paren) != Some(&b'(') {
                search_from = after_call;
                continue;
            }
            let Some(close_paren) = find_flowchart_elk_matching_paren(spec, open_paren) else {
                search_from = open_paren + 1;
                continue;
            };

            let args_slice = &spec[(open_paren + 1)..close_paren];
            let use_last_template =
                call == "renderGraph" && args_slice.trim_start().starts_with('[');
            let extracted = if use_last_template {
                extract_flowchart_elk_last_template_literal(args_slice, 0)
            } else {
                extract_flowchart_elk_first_template_literal(args_slice, 0)
            };

            if let Some((body, _end_rel)) = extracted {
                let case_name = current_it
                    .map(|it| it.name.clone())
                    .unwrap_or_else(|| "example".to_string());
                let test_slug = clamp_flowchart_elk_slug(slugify_flowchart_elk(&case_name), 64);
                let flowchart_elk_source = body.contains("flowchart-elk");
                let elk_config_source = body.contains("layout: elk")
                    || body.contains("layout: 'elk'")
                    || args_slice.contains("layout: 'elk'")
                    || args_slice.contains("layout: \"elk\"")
                    || args_slice.contains("defaultRenderer: 'elk'")
                    || args_slice.contains("defaultRenderer: \"elk\"");
                if flowchart_elk_source || elk_config_source {
                    cases.push(FlowchartElkSpecCase {
                        case_number: idx_in_file + 1,
                        test_name: case_name,
                        stem: format!(
                            "upstream_cypress_{source_slug}_{test_slug}_{case_index:03}",
                            case_index = idx_in_file + 1
                        ),
                        layout_body_key: canonical_flowchart_elk_layout_body_key(&body),
                        call,
                        snapshot: call == "imgSnapshotTest",
                    });
                }
                idx_in_file += 1;
                search_from = close_paren + 1;
                continue;
            }

            search_from = close_paren + 1;
        }
    }

    Ok(cases)
}

#[derive(Debug)]
struct FlowchartElkItPos {
    pos: usize,
    name: String,
    skipped: bool,
}

fn collect_flowchart_elk_it_positions(spec: &str) -> Vec<FlowchartElkItPos> {
    let Ok(re) = Regex::new(r#"\b(it|it\.skip)\s*\(\s*'([^']*)'"#) else {
        return Vec::new();
    };
    re.captures_iter(spec)
        .filter_map(|caps| {
            let matched = caps.get(0)?;
            Some(FlowchartElkItPos {
                pos: matched.start(),
                name: caps.get(2)?.as_str().to_string(),
                skipped: caps.get(1)?.as_str() == "it.skip",
            })
        })
        .collect()
}

fn flowchart_elk_test_at(
    it_positions: &[FlowchartElkItPos],
    abs: usize,
) -> Option<&FlowchartElkItPos> {
    let mut current = None;
    for it in it_positions {
        if it.pos > abs {
            break;
        }
        if it.pos < abs {
            current = Some(it);
        }
    }
    current
}

fn is_flowchart_elk_ident_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
}

fn is_flowchart_elk_ws_byte(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

fn find_flowchart_elk_call(text: &str, needle: &str, from: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let needle_bytes = needle.as_bytes();
    let mut i = from;
    while i + needle_bytes.len() <= bytes.len() {
        if &bytes[i..i + needle_bytes.len()] == needle_bytes {
            let before_ok = i == 0 || !is_flowchart_elk_ident_byte(bytes[i - 1]);
            let after = i + needle_bytes.len();
            let after_ok = after >= bytes.len() || !is_flowchart_elk_ident_byte(bytes[after]);
            if before_ok && after_ok {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

fn find_flowchart_elk_matching_paren(text: &str, open_paren: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    if bytes.get(open_paren) != Some(&b'(') {
        return None;
    }

    let mut mode = JsScanMode::Normal;
    let mut depth: i32 = 1;
    let mut escaped = false;
    let mut i = open_paren + 1;
    while i < bytes.len() {
        let byte = bytes[i];
        match mode {
            JsScanMode::Normal => {
                if byte == b'/' && bytes.get(i + 1) == Some(&b'/') {
                    mode = JsScanMode::LineComment;
                    i += 2;
                    continue;
                }
                if byte == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    mode = JsScanMode::BlockComment;
                    i += 2;
                    continue;
                }
                if byte == b'\'' {
                    mode = JsScanMode::SingleQuote;
                    escaped = false;
                } else if byte == b'"' {
                    mode = JsScanMode::DoubleQuote;
                    escaped = false;
                } else if byte == b'`' {
                    mode = JsScanMode::Template;
                    escaped = false;
                } else if byte == b'(' {
                    depth += 1;
                } else if byte == b')' {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                }
                i += 1;
            }
            JsScanMode::SingleQuote => {
                update_js_string_mode(byte, b'\'', &mut mode, &mut escaped);
                i += 1;
            }
            JsScanMode::DoubleQuote => {
                update_js_string_mode(byte, b'"', &mut mode, &mut escaped);
                i += 1;
            }
            JsScanMode::Template => {
                update_js_string_mode(byte, b'`', &mut mode, &mut escaped);
                i += 1;
            }
            JsScanMode::LineComment => {
                if byte == b'\n' {
                    mode = JsScanMode::Normal;
                }
                i += 1;
            }
            JsScanMode::BlockComment => {
                if byte == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    mode = JsScanMode::Normal;
                    i += 2;
                } else {
                    i += 1;
                }
            }
        }
    }
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JsScanMode {
    Normal,
    SingleQuote,
    DoubleQuote,
    Template,
    LineComment,
    BlockComment,
}

fn update_js_string_mode(byte: u8, quote: u8, mode: &mut JsScanMode, escaped: &mut bool) {
    if *escaped {
        *escaped = false;
    } else if byte == b'\\' {
        *escaped = true;
    } else if byte == quote {
        *mode = JsScanMode::Normal;
    }
}

fn extract_flowchart_elk_first_template_literal(
    input: &str,
    start: usize,
) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == b'`' {
            return parse_flowchart_elk_template_literal(input, i);
        }
        i += 1;
    }
    None
}

fn extract_flowchart_elk_last_template_literal(
    input: &str,
    start: usize,
) -> Option<(String, usize)> {
    let mut cursor = start;
    let mut last = None;
    while let Some((value, end)) = extract_flowchart_elk_first_template_literal(input, cursor) {
        last = Some((value, end));
        cursor = end;
    }
    last
}

fn parse_flowchart_elk_template_literal(input: &str, start: usize) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    if bytes.get(start) != Some(&b'`') {
        return None;
    }
    let mut out = String::new();
    let mut escaped = false;
    let mut i = start + 1;
    while i < bytes.len() {
        let byte = bytes[i];
        if escaped {
            match byte {
                b'n' => out.push('\n'),
                b'r' => out.push('\r'),
                b't' => out.push('\t'),
                b'`' => out.push('`'),
                b'\\' => out.push('\\'),
                _ => out.push(byte as char),
            }
            escaped = false;
            i += 1;
            continue;
        }
        if byte == b'\\' {
            escaped = true;
            i += 1;
            continue;
        }
        if byte == b'`' {
            return Some((out, i + 1));
        }
        out.push(byte as char);
        i += 1;
    }
    None
}

fn slugify_flowchart_elk(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut prev_us = false;
    for ch in input.chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            prev_us = false;
        } else if !prev_us {
            out.push('_');
            prev_us = true;
        }
    }
    while out.starts_with('_') {
        out.remove(0);
    }
    while out.ends_with('_') {
        out.pop();
    }
    if out.is_empty() {
        "untitled".to_string()
    } else {
        out
    }
}

fn clamp_flowchart_elk_slug(mut slug: String, max_len: usize) -> String {
    if slug.len() > max_len {
        slug.truncate(max_len);
        while slug.ends_with('_') {
            slug.pop();
        }
    }
    if slug.is_empty() {
        "untitled".to_string()
    } else {
        slug
    }
}

fn canonical_flowchart_elk_layout_body_key(input: &str) -> String {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let body = strip_flowchart_elk_yaml_frontmatter(normalized.trim_start())
        .trim_matches(|ch: char| ch.is_whitespace())
        .replace("flowchart-elk", "flowchart");

    body.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn strip_flowchart_elk_yaml_frontmatter(input: &str) -> &str {
    let mut pieces = input.split_inclusive('\n');
    let Some(first_piece) = pieces.next() else {
        return input;
    };
    let first_line = first_piece.trim_end_matches(['\n', '\r']);
    if first_line.trim() != "---" {
        return input;
    }

    let mut consumed = first_piece.len();
    for piece in pieces {
        consumed += piece.len();
        let line = piece.trim_end_matches(['\n', '\r']);
        if line.trim() == "---" {
            return &input[consumed..];
        }
    }

    input
}

fn collect_flowchart_root_pin_ids() -> std::collections::BTreeSet<String> {
    let path = crate::cmd::workspace_root()
        .join("crates")
        .join("merman-render")
        .join("src")
        .join("generated")
        .join("flowchart_root_overrides_11_12_2.rs");
    let Ok(src) = fs::read_to_string(path) else {
        return std::collections::BTreeSet::new();
    };
    let Ok(re) = Regex::new(r#""((?:stress|upstream)_[^"]+)""#) else {
        return std::collections::BTreeSet::new();
    };
    re.captures_iter(&src)
        .filter_map(|caps| caps.get(1).map(|m| m.as_str().to_string()))
        .collect()
}

fn parse_flowchart_elk_backend(
    raw: Option<&str>,
) -> Result<merman_render::FlowchartElkBackend, XtaskError> {
    match raw.map(str::trim) {
        Some("compat") => Ok(merman_render::FlowchartElkBackend::Compat),
        Some("source-ported" | "source_ported" | "source") => {
            Ok(merman_render::FlowchartElkBackend::SourcePorted)
        }
        _ => Err(XtaskError::Usage),
    }
}

fn flowchart_elk_backend_name(backend: merman_render::FlowchartElkBackend) -> &'static str {
    match backend {
        merman_render::FlowchartElkBackend::Compat => "compat",
        merman_render::FlowchartElkBackend::SourcePorted => "source-ported",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_flowchart_elk_layout_body_key, collect_flowchart_elk_spec_snapshot_cases,
        compare_flowchart_svgs,
    };

    #[test]
    fn source_backed_elk_probe_matches_html_demo_fixture() {
        let out_path = crate::cmd::target_root()
            .join("compare")
            .join("xtask-tests")
            .join("flowchart_elk_demo_probe_sourceported.md");

        compare_flowchart_svgs(vec![
            "--filter".to_string(),
            "upstream_html_demos_flowchart_elk_flowchart_elk_001".to_string(),
            "--include-elk-probes".to_string(),
            "--flowchart-elk-backend".to_string(),
            "source-ported".to_string(),
            "--check-dom".to_string(),
            "--dom-mode".to_string(),
            "parity".to_string(),
            "--dom-decimals".to_string(),
            "3".to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
        ])
        .expect("source-backed ELK probe should match the pinned HTML demo fixture");

        let report = std::fs::read_to_string(&out_path).expect("probe report should be written");
        assert!(report.contains("All fixtures matched."));
    }

    #[test]
    fn forced_elk_fixtures_require_source_ported_backend() {
        let err = compare_flowchart_svgs(vec!["--force-elk-fixture".to_string()])
            .expect_err("forced ELK fixture diagnostics should not run on the compat backend");

        assert!(
            err.to_string()
                .contains("`--force-elk-fixture` requires `--flowchart-elk-backend source-ported`")
        );
    }

    #[test]
    fn flowchart_elk_coverage_collector_tracks_snapshot_and_render_graph_cases() {
        let spec = r#"
it('first elk snapshot', () => {
  imgSnapshotTest(cy, `flowchart-elk
    A --> B`);
});

it.skip('skipped elk snapshot', () => {
  imgSnapshotTest(cy, `flowchart-elk
    skipped --> ignored`);
});

it('renderGraph elk config', () => {
  renderGraph([
    'fixture',
    `flowchart LR
      C --> D`
  ], { layout: 'elk' });
});

it('renderGraph defaultRenderer elk config', () => {
  renderGraph(
    `flowchart TD
      E --> F`,
    { flowchart: { defaultRenderer: 'elk' } }
  );
});
"#;

        let cases = collect_flowchart_elk_spec_snapshot_cases(spec)
            .expect("inline flowchart-elk spec should parse");

        assert_eq!(cases.len(), 3);
        assert_eq!(cases[0].case_number, 1);
        assert_eq!(cases[0].test_name, "first elk snapshot");
        assert_eq!(
            cases[0].stem,
            "upstream_cypress_flowchart_elk_spec_first_elk_snapshot_001"
        );
        assert_eq!(cases[0].call, "imgSnapshotTest");
        assert!(cases[0].snapshot);
        assert_eq!(cases[0].layout_body_key, "flowchart\nA --> B");

        assert_eq!(cases[1].case_number, 2);
        assert_eq!(cases[1].test_name, "renderGraph elk config");
        assert_eq!(
            cases[1].stem,
            "upstream_cypress_flowchart_elk_spec_rendergraph_elk_config_002"
        );
        assert_eq!(cases[1].call, "renderGraph");
        assert!(!cases[1].snapshot);
        assert_eq!(cases[1].layout_body_key, "flowchart LR\nC --> D");

        assert_eq!(cases[2].case_number, 3);
        assert_eq!(cases[2].test_name, "renderGraph defaultRenderer elk config");
        assert_eq!(
            cases[2].stem,
            "upstream_cypress_flowchart_elk_spec_rendergraph_defaultrenderer_elk_config_003"
        );
        assert_eq!(cases[2].call, "renderGraph");
        assert!(!cases[2].snapshot);
        assert_eq!(cases[2].layout_body_key, "flowchart TD\nE --> F");
    }

    #[test]
    fn flowchart_elk_layout_body_key_tracks_equivalent_layout_inputs() {
        let flowchart_elk = r#"
---
config:
  htmlLabels: true
---
flowchart-elk LR
  A --> B
"#;
        let default_renderer = r#"
flowchart LR
    A --> B
"#;

        assert_eq!(
            canonical_flowchart_elk_layout_body_key(flowchart_elk),
            canonical_flowchart_elk_layout_body_key(default_renderer)
        );
    }
}
