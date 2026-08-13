//! Per-diagram SVG compare commands.

use crate::XtaskError;
use crate::cmd::compare::{
    CompareFixtureResult, CompareHarnessOptions, CompareRequest, CompareRunFailure,
    CompareRunOptions, CompareRunResult, DEFAULT_LABEL_DELTA_REPORT_LIMIT,
    DEFAULT_ROOT_DELTA_REPORT_LIMIT, DiagramVerificationFact, LabelDeltaReportLimit,
    LabelMetricDelta, ObservedNodeMathRenderer, ObservedRenderOperations, RootCoverageSummary,
    RootDelta, RootDeltaReportLimit, RootEvidencePolicy, begin_required_math_evidence,
    browser_measured_math_root_note, collect_label_metric_deltas,
    comparison_mode_for_browser_measured_math, finish_required_math_evidence,
    parse_label_delta_report_limit, parse_root_delta_report_limit, record_fixture_root_evidence,
    render_semantic_svg, run_svg_compare, sanitize_svg_id, source_requires_math,
    svg_compare_engine_with_site_config, svg_request, write_compare_result_section,
    write_label_deltas_report, write_notes_section, write_root_deltas_report,
    write_verification_policy_metadata,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FlowchartUpstreamTrust {
    PinnedCanonical,
    UntrustedCustom,
}

impl FlowchartUpstreamTrust {
    fn provenance_label(self, filter: Option<&str>) -> &'static str {
        match (self, filter) {
            (Self::PinnedCanonical, None) => "pinned canonical (complete family validated)",
            (Self::PinnedCanonical, Some(_)) => "pinned canonical (selected fixtures validated)",
            (Self::UntrustedCustom, _) => "untrusted custom (debug only)",
        }
    }
}

fn classify_flowchart_upstream_dir(upstream_dir: &Path) -> FlowchartUpstreamTrust {
    let canonical_dir = crate::cmd::fixtures_root()
        .join("upstream-svgs")
        .join("flowchart");
    if upstream_dir == canonical_dir
        || fs::canonicalize(upstream_dir)
            .ok()
            .zip(fs::canonicalize(canonical_dir).ok())
            .is_some_and(|(upstream, canonical)| upstream == canonical)
    {
        FlowchartUpstreamTrust::PinnedCanonical
    } else {
        FlowchartUpstreamTrust::UntrustedCustom
    }
}

fn write_flowchart_upstream_metadata(
    report: &mut String,
    upstream_dir: &Path,
    filter: Option<&str>,
) {
    let upstream_glob = upstream_dir.join("*.svg");
    let provenance = classify_flowchart_upstream_dir(upstream_dir).provenance_label(filter);
    let _ = writeln!(report, "- Upstream: `{}`", upstream_glob.display());
    let _ = writeln!(report, "- Upstream provenance: `{provenance}`");
}

pub(super) fn compare_flowchart_args(
    fact: DiagramVerificationFact,
    args: Vec<String>,
) -> Result<(), XtaskError> {
    let mut out_path: Option<PathBuf> = None;
    let mut fixtures_root_arg: Option<PathBuf> = None;
    let mut upstream_root_arg: Option<PathBuf> = None;
    let mut filter: Option<String> = None;
    let mut check_dom: bool = false;
    let mut report_root: bool = false;
    let mut root_report_limit = DEFAULT_ROOT_DELTA_REPORT_LIMIT;
    let mut report_label: bool = false;
    let mut label_report_limit = DEFAULT_LABEL_DELTA_REPORT_LIMIT;
    let mut dom_decimals: u32 = 3;
    let mut dom_mode = fact.default_dom_mode.to_string();
    let mut text_measurer: String = "vendored".to_string();
    let mut force_elk_fixture: bool = false;

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
            "--report-root-all" => {
                report_root = true;
                root_report_limit = RootDeltaReportLimit::All;
            }
            "--report-label" => report_label = true,
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
                    .unwrap_or_else(|| fact.default_dom_mode.to_string());
            }
            "--text-measurer" => {
                i += 1;
                text_measurer = args
                    .get(i)
                    .map(|s| s.trim().to_ascii_lowercase())
                    .unwrap_or_else(|| "deterministic".to_string());
            }
            "--force-elk-fixture" => force_elk_fixture = true,
            "--help" | "-h" => return Err(XtaskError::Usage),
            _ => return Err(XtaskError::Usage),
        }
        i += 1;
    }

    run_flowchart_compare(
        fact,
        FlowchartCompareRequest {
            common: CompareRequest {
                out_path,
                filter,
                check_dom,
                dom_mode: Some(dom_mode),
                dom_decimals: Some(dom_decimals),
                report_root,
                root_report_limit: Some(root_report_limit),
                flowchart_text_measurer: Some(text_measurer),
                ..CompareRequest::default()
            },
            fixtures_root: fixtures_root_arg,
            upstream_root: upstream_root_arg,
            report_label,
            label_report_limit,
            force_elk_fixture,
        },
    )
    .map(|_| ())
    .map_err(CompareRunFailure::into_error)
}

pub(super) fn compare_flowchart_request(
    fact: DiagramVerificationFact,
    request: CompareRequest,
) -> CompareRunResult {
    run_flowchart_compare(
        fact,
        FlowchartCompareRequest {
            common: request,
            fixtures_root: None,
            upstream_root: None,
            report_label: false,
            label_report_limit: DEFAULT_LABEL_DELTA_REPORT_LIMIT,
            force_elk_fixture: false,
        },
    )
}

struct FlowchartCompareRequest {
    common: CompareRequest,
    fixtures_root: Option<PathBuf>,
    upstream_root: Option<PathBuf>,
    report_label: bool,
    label_report_limit: LabelDeltaReportLimit,
    force_elk_fixture: bool,
}

fn run_flowchart_compare(
    fact: DiagramVerificationFact,
    request: FlowchartCompareRequest,
) -> CompareRunResult {
    let tools_root = crate::cmd::mermaid_cli_root();
    let toolchain_read_guard = crate::cmd::acquire_upstream_svg_toolchain_read_guard(&tools_root)
        .map_err(CompareRunFailure::without_evidence)?;
    let math_renderer = toolchain_read_guard.node_katex_math_renderer();
    run_flowchart_compare_with_math_renderer(fact, request, math_renderer)
}

fn run_flowchart_compare_with_math_renderer(
    fact: DiagramVerificationFact,
    request: FlowchartCompareRequest,
    flowchart_math_renderer: Option<Arc<dyn merman::svg::MathRenderer + Send + Sync>>,
) -> CompareRunResult {
    let FlowchartCompareRequest {
        common,
        fixtures_root: fixtures_root_arg,
        upstream_root: upstream_root_arg,
        report_label,
        label_report_limit,
        force_elk_fixture,
    } = request;
    let out_path = common.out_path.clone();
    let filter = common.filter.clone();
    let check_dom = common.check_dom;
    let report_root = common.report_root;
    let root_report_limit = common
        .root_report_limit
        .unwrap_or(DEFAULT_ROOT_DELTA_REPORT_LIMIT);
    let dom_decimals = common.dom_decimals.unwrap_or(3);
    let dom_mode = common
        .dom_mode
        .clone()
        .unwrap_or_else(|| fact.default_dom_mode.to_string());
    let requested_dom_mode = crate::svgdom::DomMode::parse(&dom_mode);
    let text_measurer = common
        .flowchart_text_measurer
        .clone()
        .unwrap_or_else(|| "vendored".to_string());

    let should_report_root =
        report_root || matches!(dom_mode.trim(), "parity-root" | "parity_root");
    let engine = svg_compare_engine_with_site_config(serde_json::json!({ "handDrawnSeed": 1 }));
    let layout_opts = merman_render::LayoutOptions::default();
    let observed_math_renderer = flowchart_math_renderer
        .clone()
        .map(ObservedNodeMathRenderer::new);
    let text_measurement = match text_measurer.as_str() {
        "vendored" | "vendored-font" | "vendored-font-metrics" => {
            merman::svg::TextMeasurementPolicy::parity()
        }
        "deterministic" => merman::svg::TextMeasurementPolicy::deterministic(),
        other => {
            return Err(CompareRunFailure::without_evidence(
                XtaskError::SvgCompareFailed(format!(
                    "unsupported Flowchart text measurer: {other}"
                )),
            ));
        }
    };
    let mut environment = merman::svg::RenderEnvironment::deterministic()
        .with_text_measurement_policy(text_measurement);
    if let Some(renderer) = observed_math_renderer.clone() {
        environment = environment.with_math_renderer(renderer);
    }
    let observed_operations = ObservedRenderOperations::from_environment(&environment)
        .map_err(CompareRunFailure::without_evidence)?;
    let mut state = FlowchartCompareState {
        root_deltas: Vec::new(),
        root_coverage: RootCoverageSummary::default(),
        label_deltas: Vec::new(),
        observed_operations,
    };

    run_svg_compare(
        CompareHarnessOptions {
            run: CompareRunOptions {
                diagram: fact.diagram,
                out_path,
                filter: filter.as_deref(),
                check_dom,
                dom_mode: &dom_mode,
                dom_decimals,
            },
            fixtures_root: fixtures_root_arg,
            upstream_root: upstream_root_arg,
        },
        &mut state,
        |_, report, paths, options| {
            let _ = writeln!(report, "# {} SVG Comparison\n", fact.report_title);
            write_flowchart_upstream_metadata(report, &paths.upstream_dir, options.filter);
            let _ = writeln!(
                report,
                "- Command: `{}`\n- Mode: `{}`\n- Decimals: `{}`\n- Text measurer: `{}`\n- Math renderer: `{}`\n- Forced ELK fixtures: `{}`\n",
                fact.command,
                options.dom_mode,
                options.dom_decimals,
                text_measurer,
                if flowchart_math_renderer.is_some() {
                    "node-katex"
                } else {
                    "none"
                },
                if force_elk_fixture {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            write_verification_policy_metadata(
                report,
                &common,
                fact,
                options.dom_mode,
                should_report_root,
            );
            report.push('\n');
        },
        |_, stem, _| {
            crate::cmd::upstream_svg_baseline_skip_reason(fact.diagram, stem).map(str::to_string)
        },
        |state, input| {
            let diagram_id = if input.stem.ends_with("_katex") {
                roxmltree::Document::parse(input.upstream_svg)
                    .ok()
                    .and_then(|doc| doc.root_element().attribute("id").map(str::to_string))
                    .filter(|id| !id.trim().is_empty())
                    .unwrap_or_else(|| sanitize_svg_id(input.stem))
            } else {
                sanitize_svg_id(input.stem)
            };

            let fixture_engine = match input.site_config.clone() {
                Some(site_config) => engine.clone().with_site_config(site_config),
                None => engine.clone(),
            };
            let renderer = merman::Renderer::new()
                .with_engine(fixture_engine)
                .with_parse_options(fact.parse_policy.options());
            let semantic =
                match renderer.prepare_semantic(input.text, merman::OperationControl::new()) {
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
            let flowchart_layout_elk = semantic.metadata().effective_config.get_str("layout")
                == Some("elk")
                || semantic
                    .metadata()
                    .effective_config
                    .get_str("flowchart.defaultRenderer")
                    == Some("elk");
            if (semantic.metadata().diagram_type == "flowchart-elk" || flowchart_layout_elk)
                && !crate::cmd::flowchart_elk_svg_parity_admitted(input.stem)
                && !force_elk_fixture
                && let Some(reason) = crate::cmd::flowchart_elk_svg_parity_skip_reason(input.stem)
            {
                return Ok(CompareFixtureResult::Skipped {
                    reason: reason.to_string(),
                });
            }
            let requires_math = source_requires_math(
                input.fixture_path,
                &renderer,
                input.text,
                svg_request(environment.clone(), layout_opts.clone(), None),
            )?;
            let required_math_evidence_before = requires_math
                .then(|| {
                    begin_required_math_evidence(input.stem, observed_math_renderer.as_deref())
                })
                .transpose()?;

            if semantic.semantic_kind() != "flowchart" {
                return Err(format!(
                    "unexpected render family for {}: {}",
                    input.fixture_path.display(),
                    semantic.semantic_kind()
                ));
            }

            let rendered = match render_semantic_svg(
                semantic,
                svg_request(environment.clone(), layout_opts.clone(), Some(diagram_id)),
            ) {
                Ok(v) => v,
                Err(err) => {
                    return Err(format!(
                        "render failed for {}: {err}",
                        input.fixture_path.display()
                    ));
                }
            };
            let render_evidence = state
                .observed_operations
                .observe(input.stem, rendered.evidence())?;
            let local_svg = rendered.svg().to_owned();
            let mut notes = Vec::new();
            let browser_measured_math = if let Some(before) = required_math_evidence_before {
                let observed = observed_math_renderer
                    .as_deref()
                    .expect("required math evidence checked renderer availability");
                let evidence = finish_required_math_evidence(input.stem, observed, before)?;
                notes.push(format!(
                    "observed {}: Node KaTeX successful renders={} browser measurements={}",
                    input.stem,
                    evidence.successful_renders(),
                    evidence.successful_measurements()
                ));
                true
            } else {
                false
            };

            let fixture_dom_mode = comparison_mode_for_browser_measured_math(
                requested_dom_mode,
                check_dom,
                browser_measured_math,
            );
            let dom_mode_override =
                (fixture_dom_mode != requested_dom_mode).then_some(fixture_dom_mode);
            if dom_mode_override.is_some() {
                notes.push(browser_measured_math_root_note(input.stem));
            }

            let mut issues = Vec::new();
            if report_label {
                match collect_label_metric_deltas(input.stem, input.upstream_svg, &local_svg) {
                    Ok(mut rows) => state.label_deltas.append(&mut rows),
                    Err(e) => {
                        issues.push(format!("label metric parse failed for {}: {e}", input.stem))
                    }
                }
            }

            let parity_root_coverage =
                check_dom && requested_dom_mode == crate::svgdom::DomMode::ParityRoot;
            if let Err(error) = record_fixture_root_evidence(
                &mut state.root_coverage,
                &mut state.root_deltas,
                input.stem,
                input.upstream_svg,
                &local_svg,
                RootEvidencePolicy {
                    parity_root_requested: parity_root_coverage,
                    browser_math_dimensions_are_diagnostic: dom_mode_override.is_some(),
                    report_delta: should_report_root,
                },
            ) {
                issues.push(error);
            }

            Ok(match dom_mode_override {
                None => CompareFixtureResult::Rendered {
                    render_evidence,
                    local_svg,
                    compare_dom: true,
                    issues,
                    notes,
                },
                dom_mode_override => CompareFixtureResult::RenderedWithPolicy {
                    render_evidence,
                    local_svg,
                    compare_dom: true,
                    compare_svg_when_dom_disabled: false,
                    dom_mode_override,
                    issues,
                    notes,
                },
            })
        },
        |_, _, _| {},
        |state, report, paths, options, failures, notes| {
            state.observed_operations.write_report(report);
            write_compare_result_section(report, options.check_dom, failures, &paths.out_svg_dir);
            write_notes_section(report, notes);
            if check_dom && requested_dom_mode == crate::svgdom::DomMode::ParityRoot {
                state.root_coverage.write_report(report);
            }
            if should_report_root {
                write_root_deltas_report(report, &mut state.root_deltas[..], root_report_limit);
            }
            if report_label {
                write_label_deltas_report(report, &mut state.label_deltas[..], label_report_limit);
            }
        },
    )
}

struct FlowchartCompareState {
    root_deltas: Vec<RootDelta>,
    root_coverage: RootCoverageSummary,
    label_deltas: Vec<LabelMetricDelta>,
    observed_operations: ObservedRenderOperations,
}

pub(crate) fn check_flowchart_elk_parity(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }

    let fact = super::diagram_verification_fact("flowchart")
        .copied()
        .expect("Flowchart must have a verification fact");
    let mut failures = Vec::new();
    for stem in crate::cmd::flowchart_elk_svg_parity_stems() {
        let out_path = crate::cmd::target_root()
            .join("compare")
            .join("flowchart_elk_parity")
            .join(format!("{stem}.md"));
        let result = compare_flowchart_args(
            fact,
            vec![
                "--filter".to_string(),
                (*stem).to_string(),
                "--check-dom".to_string(),
                "--dom-mode".to_string(),
                "parity".to_string(),
                "--dom-decimals".to_string(),
                "3".to_string(),
                "--out".to_string(),
                out_path.display().to_string(),
            ],
        );
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

pub(crate) fn audit_flowchart_elk_parity_coverage(args: Vec<String>) -> Result<(), XtaskError> {
    if !args.is_empty() {
        return Err(XtaskError::Usage);
    }

    let workspace_root = crate::cmd::workspace_root();
    let manifest = crate::cmd::load_committed_flowchart_elk_manifest(&workspace_root)
        .map_err(XtaskError::AlignmentCheckFailed)?;
    let manifest_failures = crate::cmd::validate_flowchart_elk_manifest(&workspace_root, &manifest);
    if !manifest_failures.is_empty() {
        return Err(XtaskError::AlignmentCheckFailed(
            manifest_failures.join("\n"),
        ));
    }
    let source_spec = manifest
        .collection
        .source
        .specs
        .first()
        .map(|spec| spec.path.as_str())
        .unwrap_or("<missing source spec>");

    let fixture_dir = crate::cmd::fixtures_root().join("flowchart");
    let upstream_svg_dir = crate::cmd::fixtures_root()
        .join("upstream-svgs")
        .join("flowchart");

    let cases = &manifest.entries;
    let admitted = crate::cmd::flowchart_elk_svg_parity_stems();
    let mut admitted_layout_body_keys: BTreeMap<String, String> = BTreeMap::new();
    for stem in admitted {
        let fixture_path = fixture_dir.join(format!("{stem}.mmd"));
        let svg_path = upstream_svg_dir.join(format!("{stem}.svg"));
        if !svg_path.is_file() {
            continue;
        }
        let text = match fs::read_to_string(&fixture_path) {
            Ok(text) => text,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => continue,
            Err(source) => {
                return Err(XtaskError::ReadFile {
                    path: fixture_path.display().to_string(),
                    source,
                });
            }
        };
        admitted_layout_body_keys
            .entry(crate::util::sha256_hex(
                canonical_flowchart_elk_layout_body_key(&text).as_bytes(),
            ))
            .or_insert_with(|| (*stem).to_string());
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
    let mut uncovered_layout_body_groups: BTreeMap<
        String,
        Vec<&crate::cmd::FlowchartElkCollectionEntry>,
    > = BTreeMap::new();

    for case in cases {
        let fixture_path = fixture_dir.join(format!("{}.mmd", case.stem));
        let svg_path = upstream_svg_dir.join(format!("{}.svg", case.stem));
        let has_fixture = fixture_path.is_file();
        let has_svg = svg_path.is_file();
        let is_admitted = admitted.contains(&case.stem.as_str());
        let has_exact_parity_baseline = is_admitted && has_fixture && has_svg;
        let covered_by_layout_body = admitted_layout_body_keys
            .get(&case.layout_body_sha256)
            .map(String::as_str);

        unique_layout_body_keys.insert(case.layout_body_sha256.clone());
        if covered_by_layout_body.is_some() {
            covered_layout_body_keys.insert(case.layout_body_sha256.clone());
        } else {
            uncovered_layout_body_groups
                .entry(case.layout_body_sha256.clone())
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
        if has_exact_parity_baseline {
            admitted_count += 1;
        } else if let Some(representative) = covered_by_layout_body {
            duplicate_covered.push((case, representative));
        }
        if !is_admitted {
            not_admitted.push(case);
        }
    }

    let uncovered_layout_body_count =
        unique_layout_body_keys.len() - covered_layout_body_keys.len();

    println!("Flowchart ELK parity coverage");
    println!("spec: {source_spec}");
    let duplicate_covered_count = duplicate_covered.len();
    println!("ELK render calls: {}", cases.len());
    println!(
        "exact render calls covered by parity fixtures or duplicate layout bodies: {}",
        admitted_count + duplicate_covered_count
    );
    println!("dedicated fixtures present: {fixture_count}");
    println!("dedicated upstream SVGs present: {upstream_svg_count}");
    println!("canonical parity fixtures: {admitted_count}");
    println!(
        "dedicated fixture gaps covered by duplicate layout body: {}",
        missing
            .iter()
            .filter(|case| admitted_layout_body_keys.contains_key(&case.layout_body_sha256))
            .count()
    );
    println!(
        "dedicated upstream SVG gaps covered by duplicate layout body: {}",
        no_upstream_svg
            .iter()
            .filter(|case| admitted_layout_body_keys.contains_key(&case.layout_body_sha256))
            .count()
    );
    println!(
        "unadmitted exact calls covered by duplicate layout body: {}",
        duplicate_covered
            .iter()
            .filter(|(case, _)| !admitted.contains(&case.stem.as_str()))
            .count()
    );
    println!("unique layout bodies: {}", unique_layout_body_keys.len());
    println!(
        "unique layout bodies covered by parity fixtures: {}",
        covered_layout_body_keys.len()
    );
    println!("uncovered unique layout bodies: {uncovered_layout_body_count}");

    if !duplicate_covered.is_empty() {
        println!();
        println!("Non-canonical calls covered through an admitted duplicate layout body:");
        for (case, representative) in &duplicate_covered {
            println!(
                "- {} {} [{}{}]",
                case.source_ordinal,
                case.test_name,
                case.call,
                if case.snapshot() { ", snapshot" } else { "" }
            );
            println!("  stem: {}", case.stem);
            println!("  covered_by: {representative}");
        }
    }

    if !uncovered_layout_body_groups.is_empty() {
        println!();
        println!("Uncovered unique layout bodies:");
        let mut groups = uncovered_layout_body_groups.values().collect::<Vec<_>>();
        groups.sort_by_key(|group| group[0].source_ordinal);
        for group in groups {
            let representative = group[0];
            println!(
                "- {} {} [{}{}]",
                representative.source_ordinal,
                representative.test_name,
                representative.call,
                if representative.snapshot() {
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
        println!("Dedicated fixture gaps:");
        for case in &missing {
            println!(
                "- {} {} [{}{}]",
                case.source_ordinal,
                case.test_name,
                case.call,
                if case.snapshot() { ", snapshot" } else { "" }
            );
            println!("  stem: {}", case.stem);
            if let Some(representative) = admitted_layout_body_keys.get(&case.layout_body_sha256) {
                println!("  covered_by: {representative}");
            }
        }
    }

    if !not_admitted.is_empty() {
        println!();
        println!("Unadmitted exact calls:");
        for case in &not_admitted {
            println!(
                "- {} {} [{}{}]",
                case.source_ordinal,
                case.test_name,
                case.call,
                if case.snapshot() { ", snapshot" } else { "" }
            );
            println!("  stem: {}", case.stem);
            if let Some(representative) = admitted_layout_body_keys.get(&case.layout_body_sha256) {
                println!("  covered_by: {representative}");
            }
        }
    }

    if !no_upstream_svg.is_empty() {
        println!();
        println!("Dedicated upstream SVG gaps:");
        for case in &no_upstream_svg {
            println!(
                "- {} {} [{}{}]",
                case.source_ordinal,
                case.test_name,
                case.call,
                if case.snapshot() { ", snapshot" } else { "" }
            );
            println!("  stem: {}", case.stem);
            if let Some(representative) = admitted_layout_body_keys.get(&case.layout_body_sha256) {
                println!("  covered_by: {representative}");
            }
        }
    }

    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FlowchartElkFixtureIdentity {
    pub(crate) stem: String,
    pub(crate) mmd_sha256: String,
    pub(crate) layout_body_key: String,
}

fn existing_flowchart_elk_source_identities() -> Result<HashMap<String, Vec<String>>, XtaskError> {
    let fixture_dir = crate::cmd::fixtures_root().join("flowchart");
    let entries = fs::read_dir(&fixture_dir).map_err(|source| XtaskError::ReadFile {
        path: fixture_dir.display().to_string(),
        source,
    })?;
    let mut identities = HashMap::<String, Vec<String>>::new();
    for entry in entries {
        let path = entry
            .map_err(|source| XtaskError::ReadFile {
                path: fixture_dir.display().to_string(),
                source,
            })?
            .path();
        let Some(stem) = path
            .extension()
            .filter(|extension| *extension == "mmd")
            .and_then(|_| path.file_stem())
            .and_then(|stem| stem.to_str())
            .filter(|stem| stem.starts_with("upstream_cypress_flowchart_elk_spec_"))
            .filter(|stem| crate::cmd::flowchart_elk_svg_parity_admitted(stem))
        else {
            continue;
        };
        let source = fs::read_to_string(&path).map_err(|source| XtaskError::ReadFile {
            path: path.display().to_string(),
            source,
        })?;
        identities
            .entry(canonical_flowchart_elk_source_body_key(&source))
            .or_default()
            .push(stem.to_string());
    }
    for stems in identities.values_mut() {
        stems.sort();
    }
    Ok(identities)
}

pub(crate) fn flowchart_elk_fixture_identity(
    test_name: &str,
    fixture_source: &str,
    source_slug: &str,
    existing_identities: &HashMap<String, Vec<String>>,
) -> FlowchartElkFixtureIdentity {
    let test_slug = clamp_flowchart_elk_slug(slugify_flowchart_elk(test_name), 64);
    let prefix = format!("upstream_cypress_{source_slug}_{test_slug}_");
    let source_body_key = canonical_flowchart_elk_source_body_key(fixture_source);
    let existing_stems = existing_identities
        .get(&source_body_key)
        .into_iter()
        .flatten()
        .filter(|stem| stem.starts_with(&prefix))
        .collect::<Vec<_>>();
    let stem = match existing_stems.as_slice() {
        [existing] => (*existing).clone(),
        _ => format!(
            "{prefix}{}",
            crate::cmd::imported_fixture_content_id(fixture_source)
        ),
    };
    FlowchartElkFixtureIdentity {
        stem,
        mmd_sha256: crate::util::sha256_hex(fixture_source.as_bytes()),
        layout_body_key: canonical_flowchart_elk_layout_body_key(fixture_source),
    }
}

pub(crate) fn flowchart_elk_source_slug() -> String {
    clamp_flowchart_elk_slug(slugify_flowchart_elk("flowchart-elk spec"), 48)
}

pub(crate) fn flowchart_elk_source_identities() -> Result<HashMap<String, Vec<String>>, XtaskError>
{
    existing_flowchart_elk_source_identities()
}

pub(crate) fn flowchart_elk_requested(body: &str, options: &serde_json::Value) -> bool {
    value_path_is_elk(options, &["layout"])
        || value_path_is_elk(options, &["flowchart", "defaultRenderer"])
        || split_flowchart_elk_yaml_frontmatter(body)
            .and_then(|(yaml, _)| serde_saphyr::from_str::<serde_json::Value>(yaml).ok())
            .is_some_and(|frontmatter| {
                value_path_is_elk(&frontmatter, &["config", "layout"])
                    || value_path_is_elk(&frontmatter, &["config", "flowchart", "defaultRenderer"])
            })
        || first_flowchart_directive(body).is_some_and(|directive| directive == "flowchart-elk")
}

fn value_path_is_elk(value: &serde_json::Value, path: &[&str]) -> bool {
    path.iter()
        .try_fold(value, |value, key| value.get(*key))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|renderer| renderer.eq_ignore_ascii_case("elk"))
}

fn first_flowchart_directive(input: &str) -> Option<&str> {
    let body = split_flowchart_elk_yaml_frontmatter(input)
        .map(|(_, body)| body)
        .unwrap_or(input);
    body.lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with("%%"))
        .and_then(|line| line.split_ascii_whitespace().next())
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
    canonical_flowchart_elk_body_key(input, true)
}

fn canonical_flowchart_elk_source_body_key(input: &str) -> String {
    canonical_flowchart_elk_body_key(input, false)
}

fn canonical_flowchart_elk_body_key(input: &str, ignore_renderer_selection: bool) -> String {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let (frontmatter, body) = split_flowchart_elk_yaml_frontmatter(&normalized)
        .map(|(yaml, body)| (Some(yaml), body))
        .unwrap_or((None, normalized.as_str()));
    let mut value = match frontmatter {
        Some(yaml) => match serde_saphyr::from_str::<serde_json::Value>(yaml) {
            Ok(serde_json::Value::Null) => serde_json::Value::Object(serde_json::Map::new()),
            Ok(value) => value,
            Err(_) => {
                return format!(
                    "frontmatter:raw:{yaml}\nbody:{}",
                    normalize_flowchart_elk_directive(body).trim_matches('\n')
                );
            }
        },
        None => serde_json::Value::Object(serde_json::Map::new()),
    };
    crate::cmd::import::canonicalize_imported_config_value(&mut value);
    let frontmatter_key = {
        if ignore_renderer_selection {
            remove_elk_renderer_path(&mut value, &["config", "layout"]);
            remove_elk_renderer_path(&mut value, &["config", "flowchart", "defaultRenderer"]);
        }
        prune_empty_json_objects(&mut value);
        if value.as_object().is_some_and(serde_json::Map::is_empty) {
            String::new()
        } else {
            serde_json::to_string(&value).unwrap_or_default()
        }
    };
    let body = normalize_flowchart_elk_directive(body)
        .trim_matches('\n')
        .to_string();
    format!("frontmatter:{frontmatter_key}\nbody:{body}")
}

fn split_flowchart_elk_yaml_frontmatter(input: &str) -> Option<(&str, &str)> {
    let mut pieces = input.split_inclusive('\n');
    let first_piece = pieces.next()?;
    let first_line = first_piece.trim_end_matches(['\n', '\r']);
    if first_line.trim() != "---" {
        return None;
    }

    let mut consumed = first_piece.len();
    for piece in pieces {
        let yaml_end = consumed;
        consumed += piece.len();
        let line = piece.trim_end_matches(['\n', '\r']);
        if line.trim() == "---" {
            return Some((&input[first_piece.len()..yaml_end], &input[consumed..]));
        }
    }

    None
}

fn remove_elk_renderer_path(value: &mut serde_json::Value, path: &[&str]) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut current = value;
    for key in parents {
        let Some(next) = current.get_mut(*key) else {
            return;
        };
        current = next;
    }
    let should_remove = current
        .get(*last)
        .and_then(serde_json::Value::as_str)
        .is_some_and(|renderer| renderer.eq_ignore_ascii_case("elk"));
    if should_remove && let Some(object) = current.as_object_mut() {
        object.remove(*last);
    }
}

fn prune_empty_json_objects(value: &mut serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => {
            object.retain(|_, child| !prune_empty_json_objects(child));
            object.is_empty()
        }
        serde_json::Value::Array(values) => {
            for child in values {
                prune_empty_json_objects(child);
            }
            false
        }
        _ => false,
    }
}

fn normalize_flowchart_elk_directive(body: &str) -> String {
    let mut normalized = String::with_capacity(body.len());
    let mut replaced = false;
    for line in body.split_inclusive('\n') {
        if !replaced {
            let trimmed = line.trim_start();
            if !trimmed.is_empty() && !trimmed.starts_with("%%") {
                let indentation = line.len() - trimmed.len();
                if trimmed
                    .strip_prefix("flowchart-elk")
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
                {
                    normalized.push_str(&line[..indentation]);
                    normalized.push_str("flowchart");
                    normalized.push_str(&trimmed["flowchart-elk".len()..]);
                    replaced = true;
                    continue;
                }
                replaced = true;
            }
        }
        normalized.push_str(line);
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::{
        FlowchartCompareRequest, FlowchartUpstreamTrust, canonical_flowchart_elk_layout_body_key,
        classify_flowchart_upstream_dir, compare_flowchart_args, render_source_svg,
        run_flowchart_compare_with_math_renderer, svg_request, write_flowchart_upstream_metadata,
    };
    use crate::cmd::compare::{
        CompareRequest, DEFAULT_LABEL_DELTA_REPORT_LIMIT, DiagramVerificationFact,
    };
    use std::path::Path;
    use std::sync::Arc;

    #[derive(Debug)]
    struct SuccessfulMathRenderer;

    impl merman::svg::MathRenderer for SuccessfulMathRenderer {
        fn render_html_label(
            &self,
            _text: &str,
            _config: &merman::MermaidConfig,
        ) -> Option<String> {
            Some("<span>math</span>".to_string())
        }

        fn measure_html_label(
            &self,
            _text: &str,
            _config: &merman::MermaidConfig,
            _style: &merman::svg::TextStyle,
            _max_width_px: Option<f64>,
            _wrap_mode: merman::svg::WrapMode,
        ) -> Option<merman::svg::TextMetrics> {
            Some(merman::svg::TextMetrics {
                width: 40.0,
                height: 20.0,
                line_count: 1,
            })
        }
    }

    fn compare_flowchart(args: Vec<String>) -> Result<(), crate::XtaskError> {
        let fact = super::super::diagram_verification_fact("flowchart")
            .copied()
            .expect("Flowchart must have a verification fact");
        compare_flowchart_args(fact, args)
    }

    fn flowchart_fact() -> DiagramVerificationFact {
        super::super::diagram_verification_fact("flowchart")
            .copied()
            .expect("Flowchart must have a verification fact")
    }

    fn write_flowchart_compare_fixture(root: &Path, stem: &str, source: &str, upstream_svg: &str) {
        let fixtures = root.join("fixtures").join("flowchart");
        let upstream = root.join("upstream").join("flowchart");
        std::fs::create_dir_all(&fixtures).expect("fixture directory should be created");
        std::fs::create_dir_all(&upstream).expect("upstream directory should be created");
        std::fs::write(fixtures.join(format!("{stem}.mmd")), source)
            .expect("fixture should be written");
        std::fs::write(upstream.join(format!("{stem}.svg")), upstream_svg)
            .expect("upstream SVG should be written");
    }

    fn render_plain_flowchart(fact: DiagramVerificationFact, stem: &str, source: &str) -> String {
        let renderer = merman::Renderer::new()
            .with_engine(super::svg_compare_engine_with_site_config(
                serde_json::json!({ "handDrawnSeed": 1 }),
            ))
            .with_parse_options(fact.parse_policy.options());
        render_source_svg(
            &renderer,
            source,
            svg_request(
                merman::svg::RenderEnvironment::deterministic()
                    .with_text_measurement_policy(merman::svg::TextMeasurementPolicy::parity()),
                merman_render::LayoutOptions::default(),
                Some(stem.to_string()),
            ),
        )
        .expect("plain Flowchart render should succeed")
        .svg()
        .to_owned()
    }

    fn flowchart_compare_request(root: &Path) -> FlowchartCompareRequest {
        FlowchartCompareRequest {
            common: CompareRequest {
                out_path: Some(root.join("report.md")),
                check_dom: true,
                dom_mode: Some("parity".to_string()),
                flowchart_text_measurer: Some("vendored".to_string()),
                ..CompareRequest::default()
            },
            fixtures_root: Some(root.join("fixtures")),
            upstream_root: Some(root.join("upstream")),
            report_label: false,
            label_report_limit: DEFAULT_LABEL_DELTA_REPORT_LIMIT,
            force_elk_fixture: false,
        }
    }

    #[test]
    fn plain_flowchart_compare_does_not_require_a_math_backend() {
        let temp = tempfile::tempdir().expect("temporary compare root");
        let fact = flowchart_fact();
        let source = "flowchart LR\n  A --> B\n";
        let upstream = render_plain_flowchart(fact, "plain", source);
        write_flowchart_compare_fixture(temp.path(), "plain", source, &upstream);

        let mut request = flowchart_compare_request(temp.path());
        request.common.filter = Some("plain".to_string());
        let evidence = run_flowchart_compare_with_math_renderer(fact, request, None)
            .expect("plain fixtures must not require a math backend");

        assert_eq!(evidence.selected_fixtures(), 1);
        assert_eq!(evidence.rendered_fixtures(), 1);
        assert_eq!(evidence.comparisons(), 1);
    }

    #[test]
    fn math_only_flowchart_compare_fails_without_a_backend() {
        let temp = tempfile::tempdir().expect("temporary compare root");
        let fact = flowchart_fact();
        let source = "flowchart LR\n  A[\"$$x + y$$\"] --> B\n";
        write_flowchart_compare_fixture(temp.path(), "math_only", source, "<svg/>");

        let failure = run_flowchart_compare_with_math_renderer(
            fact,
            flowchart_compare_request(temp.path()),
            None,
        )
        .expect_err("math fixtures must require a math backend");
        let evidence = failure.evidence();
        let message = failure.to_string();

        assert_eq!(evidence.selected_fixtures(), 1);
        assert_eq!(evidence.rendered_fixtures(), 0);
        assert_eq!(evidence.comparisons(), 0);
        assert!(message.contains("cannot compare math fixture math_only"));
        assert!(message.contains("Node KaTeX backend is unavailable"));
    }

    #[test]
    fn mixed_flowchart_selection_cannot_hide_a_missing_math_backend() {
        let temp = tempfile::tempdir().expect("temporary compare root");
        let fact = flowchart_fact();
        let plain_source = "flowchart LR\n  A --> B\n";
        let plain_upstream = render_plain_flowchart(fact, "a_plain", plain_source);
        write_flowchart_compare_fixture(temp.path(), "a_plain", plain_source, &plain_upstream);
        write_flowchart_compare_fixture(
            temp.path(),
            "b_math",
            "flowchart LR\n  A[\"$$x + y$$\"] --> B\n",
            "<svg/>",
        );

        let failure = run_flowchart_compare_with_math_renderer(
            fact,
            flowchart_compare_request(temp.path()),
            None,
        )
        .expect_err("a plain fixture's DOM evidence must not hide a math failure");
        let evidence = failure.evidence();
        let message = failure.to_string();

        assert_eq!(evidence.selected_fixtures(), 2);
        assert_eq!(evidence.rendered_fixtures(), 1);
        assert_eq!(evidence.comparisons(), 1);
        assert!(message.contains("cannot compare math fixture b_math"));
    }

    #[test]
    fn flowchart_math_backend_must_produce_fixture_scoped_evidence() {
        let temp = tempfile::tempdir().expect("temporary compare root");
        let fact = flowchart_fact();
        write_flowchart_compare_fixture(
            temp.path(),
            "declined_math",
            "flowchart LR\n  A[\"$$x + y$$\"] --> B\n",
            "<svg/>",
        );
        let declining: Arc<dyn merman::svg::MathRenderer + Send + Sync> =
            Arc::new(merman::svg::NoopMathRenderer);

        let failure = run_flowchart_compare_with_math_renderer(
            fact,
            flowchart_compare_request(temp.path()),
            Some(declining),
        )
        .expect_err("a backend that declines every call must not count as math evidence");
        let message = failure.to_string();

        assert!(message.contains("math fixture declined_math"));
        assert!(message.contains("no successful Node KaTeX browser measurement evidence"));
    }

    #[test]
    fn disabled_dom_check_does_not_activate_the_math_root_exception() {
        let temp = tempfile::tempdir().expect("temporary compare root");
        let fact = flowchart_fact();
        write_flowchart_compare_fixture(
            temp.path(),
            "math_without_dom_check",
            "flowchart LR\n  A[\"$$x + y$$\"] --> B\n",
            "<svg/>",
        );
        let mut request = flowchart_compare_request(temp.path());
        request.common.check_dom = false;
        request.common.dom_mode = Some("parity-root".to_string());

        let evidence = run_flowchart_compare_with_math_renderer(
            fact,
            request,
            Some(Arc::new(SuccessfulMathRenderer)),
        )
        .expect("a disabled DOM check must not activate root comparison policy");

        assert_eq!(evidence.selected_fixtures(), 1);
        assert_eq!(evidence.rendered_fixtures(), 1);
        assert_eq!(evidence.comparisons(), 0);
    }

    #[test]
    fn flowchart_report_marks_canonical_upstream_as_provenance_validated() {
        let canonical = crate::cmd::fixtures_root()
            .join("upstream-svgs")
            .join("flowchart");
        let explicit = canonical.join(".");
        let mut report = String::new();

        assert_eq!(
            classify_flowchart_upstream_dir(&explicit),
            FlowchartUpstreamTrust::PinnedCanonical
        );
        assert_eq!(
            FlowchartUpstreamTrust::PinnedCanonical.provenance_label(None),
            "pinned canonical (complete family validated)"
        );
        write_flowchart_upstream_metadata(&mut report, &explicit, Some("fixture"));

        assert!(report.contains(&format!(
            "- Upstream: `{}`",
            explicit.join("*.svg").display()
        )));
        assert!(
            report.contains(
                "- Upstream provenance: `pinned canonical (selected fixtures validated)`"
            )
        );
        assert!(!report.contains("untrusted custom"));
    }

    #[test]
    fn flowchart_report_marks_custom_upstream_as_untrusted() {
        let custom = crate::cmd::target_root()
            .join("compare")
            .join("custom-upstream-svgs")
            .join("flowchart");
        let mut report = String::new();

        assert_eq!(
            classify_flowchart_upstream_dir(&custom),
            FlowchartUpstreamTrust::UntrustedCustom
        );
        write_flowchart_upstream_metadata(&mut report, &custom, None);

        assert!(report.contains(&format!("- Upstream: `{}`", custom.join("*.svg").display())));
        assert!(report.contains("- Upstream provenance: `untrusted custom (debug only)`"));
        assert!(!report.contains("pinned canonical"));
        assert!(!report.contains("validated"));
    }

    #[test]
    fn flowchart_elk_parity_admission_matches_html_demo_fixture() {
        let out_path = crate::cmd::target_root()
            .join("compare")
            .join("xtask-tests")
            .join("flowchart_elk_demo_parity.md");

        compare_flowchart(vec![
            "--filter".to_string(),
            "upstream_html_demos_flowchart_elk_flowchart_elk_001".to_string(),
            "--check-dom".to_string(),
            "--dom-mode".to_string(),
            "parity".to_string(),
            "--dom-decimals".to_string(),
            "3".to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
        ])
        .expect("ELK parity admission should match the pinned HTML demo fixture");

        let report = std::fs::read_to_string(&out_path).expect("probe report should be written");
        assert!(report.contains("All fixtures matched."));
    }

    #[test]
    fn forced_flowchart_elk_fixture_diagnostics_use_the_canonical_layout() {
        let out_path = crate::cmd::target_root()
            .join("compare")
            .join("xtask-tests")
            .join("flowchart_elk_default_forced.md");

        compare_flowchart(vec![
            "--filter".to_string(),
            "upstream_html_demos_flowchart_elk_flowchart_elk_001".to_string(),
            "--force-elk-fixture".to_string(),
            "--out".to_string(),
            out_path.display().to_string(),
        ])
        .expect("forced ELK fixture diagnostics should use the canonical ELK layout");

        let report = std::fs::read_to_string(&out_path).expect("forced report should be written");
        assert!(report.contains("- Forced ELK fixtures: `enabled`"));
        assert!(!report.contains("Flowchart ELK backend"));
    }

    #[test]
    fn flowchart_elk_layout_identity_preserves_non_renderer_config() {
        let renderer_only = r#"---
config:
  layout: elk
---
flowchart LR
  A --> B
"#;
        let directive = "flowchart-elk LR\n  A --> B\n";
        assert_eq!(
            canonical_flowchart_elk_layout_body_key(renderer_only),
            canonical_flowchart_elk_layout_body_key(directive)
        );

        let html_labels = r#"---
config:
  layout: elk
  htmlLabels: true
---
flowchart LR
  A --> B
"#;
        assert_ne!(
            canonical_flowchart_elk_layout_body_key(html_labels),
            canonical_flowchart_elk_layout_body_key(directive)
        );
    }
}
