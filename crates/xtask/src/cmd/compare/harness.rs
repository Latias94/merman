//! Shared execution harness for per-diagram SVG compare commands.

use crate::XtaskError;
use crate::svgdom;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VerificationRenderPath {
    HeadlessOperationTyped,
}

impl VerificationRenderPath {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::HeadlessOperationTyped => "headless-operation-typed",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AcceptedResidualPolicy {
    #[default]
    None,
    SequenceExactDom,
    QuadrantStructureRenderability,
    RootParityExact,
}

impl AcceptedResidualPolicy {
    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::SequenceExactDom => {
                "compare-all exact fail-closed Sequence DOM residual registry"
            }
            Self::QuadrantStructureRenderability => {
                "compare-all source-backed QuadrantChart renderability correction registry"
            }
            Self::RootParityExact => "compare-all exact fail-closed root residual registry",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ParsePolicy {
    Default,
    SuppressErrors,
    Lenient,
}

impl ParsePolicy {
    pub(crate) fn options(self) -> merman::ParseOptions {
        match self {
            Self::Default => merman::ParseOptions::default(),
            Self::SuppressErrors => merman::ParseOptions {
                suppress_errors: true,
            },
            Self::Lenient => merman::ParseOptions::lenient(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderProfile {
    Standard,
    HandDrawnSeed,
    GitGraphSeed,
    SequenceMath,
    Specialist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagramIdPolicy {
    SanitizedStem,
    RawStem,
    Specialist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixtureSkipPolicy {
    None,
    UpstreamBaseline,
    UpstreamCompare,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixtureComparePolicy {
    Dom,
    DomAndRawSvgFallback,
    Specialist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FixtureReportPolicy {
    Summary,
    StatusLines,
    Specialist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DiagnosticsPolicy {
    None,
    RootDelta,
    Specialist,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpecialistHook {
    None,
    ClassV2Role,
    SequenceMath,
    FlowchartAdapter,
    ErAdapter,
    GanttAdapter,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DiagramVerificationFact {
    pub(crate) diagram: &'static str,
    pub(crate) command: &'static str,
    pub(crate) report_title: &'static str,
    pub(crate) default_dom_mode: &'static str,
    pub(crate) parse_policy: ParsePolicy,
    pub(crate) render_profile: RenderProfile,
    pub(crate) diagram_id_policy: DiagramIdPolicy,
    pub(crate) skip_policy: FixtureSkipPolicy,
    pub(crate) compare_policy: FixtureComparePolicy,
    pub(crate) report_policy: FixtureReportPolicy,
    pub(crate) diagnostics: DiagnosticsPolicy,
    pub(crate) specialist: SpecialistHook,
}

impl DiagramVerificationFact {
    pub(crate) const fn render_path(self) -> VerificationRenderPath {
        VerificationRenderPath::HeadlessOperationTyped
    }

    pub(crate) const fn supports_root_report(self) -> bool {
        matches!(self.diagnostics, DiagnosticsPolicy::RootDelta)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompareRequest {
    pub(crate) out_path: Option<PathBuf>,
    pub(crate) filter: Option<String>,
    pub(crate) check_dom: bool,
    pub(crate) dom_mode: Option<String>,
    pub(crate) dom_decimals: Option<u32>,
    pub(crate) report_root: bool,
    pub(crate) root_report_limit: Option<super::RootDeltaReportLimit>,
    pub(crate) apply_root_overrides: bool,
    pub(crate) flowchart_text_measurer: Option<String>,
    pub(crate) flowchart_elk_backend: Option<merman_render::FlowchartElkBackend>,
    pub(crate) include_elk_probes: bool,
    pub(crate) accepted_residual_policy: AcceptedResidualPolicy,
}

impl Default for CompareRequest {
    fn default() -> Self {
        Self {
            out_path: None,
            filter: None,
            check_dom: false,
            dom_mode: None,
            dom_decimals: None,
            report_root: false,
            root_report_limit: None,
            apply_root_overrides: true,
            flowchart_text_measurer: None,
            flowchart_elk_backend: None,
            include_elk_probes: false,
            accepted_residual_policy: AcceptedResidualPolicy::None,
        }
    }
}

impl CompareRequest {
    pub(crate) fn parse_for_fact(
        args: Vec<String>,
        fact: DiagramVerificationFact,
    ) -> Result<Self, XtaskError> {
        let mut request = Self::default();
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
                "--check-dom" => request.check_dom = true,
                "--dom-mode" => {
                    i += 1;
                    request.dom_mode = Some(
                        args.get(i)
                            .map(|value| value.trim().to_string())
                            .unwrap_or_else(|| fact.default_dom_mode.to_string()),
                    );
                }
                "--dom-decimals" => {
                    i += 1;
                    request.dom_decimals = Some(
                        args.get(i)
                            .and_then(|value| value.parse::<u32>().ok())
                            .unwrap_or(3),
                    );
                }
                "--report-root" if fact.supports_root_report() => request.report_root = true,
                "--report-root-all" if fact.supports_root_report() => {
                    request.report_root = true;
                    request.root_report_limit = Some(super::RootDeltaReportLimit::All);
                }
                "--report-root-limit" if fact.supports_root_report() => {
                    i += 1;
                    request.report_root = true;
                    request.root_report_limit = Some(super::parse_root_delta_report_limit(
                        args.get(i).map(String::as_str),
                    )?);
                }
                "--no-root-overrides" => request.apply_root_overrides = false,
                "--help" | "-h" => return Err(XtaskError::Usage),
                _ => return Err(XtaskError::Usage),
            }
            i += 1;
        }
        Ok(request)
    }
}

pub(crate) fn compare_render_environment(
    request: &CompareRequest,
) -> merman::render::RenderEnvironment {
    merman::render::RenderEnvironment::parity().with_root_viewport_override_policy(
        if request.apply_root_overrides {
            merman::render::RootViewportOverridePolicy::ApplyGenerated
        } else {
            merman::render::RootViewportOverridePolicy::ComputedOnly
        },
    )
}

#[derive(Default)]
struct CanonicalCompareState {
    root_deltas: Vec<super::RootDelta>,
}

#[derive(Debug, Clone)]
pub(crate) struct CompareRunOptions<'a> {
    pub(crate) diagram: &'a str,
    pub(crate) out_path: Option<PathBuf>,
    pub(crate) filter: Option<&'a str>,
    pub(crate) check_dom: bool,
    pub(crate) dom_mode: &'a str,
    pub(crate) dom_decimals: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct CompareHarnessOptions<'a> {
    pub(crate) run: CompareRunOptions<'a>,
    pub(crate) fixtures_root: Option<PathBuf>,
    pub(crate) upstream_root: Option<PathBuf>,
}

impl<'a> CompareHarnessOptions<'a> {
    pub(crate) fn new(run: CompareRunOptions<'a>) -> Self {
        Self {
            run,
            fixtures_root: None,
            upstream_root: None,
        }
    }
}

pub(crate) type CompareRunPaths = super::CompareDiagramPaths;

#[derive(Debug, Clone, Copy)]
pub(crate) struct CompareFixtureInput<'a> {
    pub(crate) stem: &'a str,
    pub(crate) fixture_path: &'a Path,
    pub(crate) upstream_svg: &'a str,
    pub(crate) text: &'a str,
    pub(crate) check_dom: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum CompareFixtureResult {
    Skipped {
        reason: String,
    },
    Rendered {
        local_svg: String,
        compare_dom: bool,
        issues: Vec<String>,
        notes: Vec<String>,
    },
    RenderedWithPolicy {
        local_svg: String,
        compare_dom: bool,
        compare_svg_when_dom_disabled: bool,
        issues: Vec<String>,
        notes: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CompareFixtureReportInput<'a> {
    pub(crate) stem: &'a str,
    pub(crate) fixture_path: &'a Path,
    pub(crate) upstream_path: &'a Path,
    pub(crate) local_out_path: &'a Path,
    pub(crate) failed: bool,
}

fn is_pinned_upstream_dir(diagram: &str, upstream_dir: &Path) -> bool {
    let pinned = crate::cmd::fixtures_root()
        .join("upstream-svgs")
        .join(diagram);
    match (fs::canonicalize(upstream_dir), fs::canonicalize(&pinned)) {
        (Ok(actual), Ok(expected)) => actual == expected,
        _ => upstream_dir == pinned,
    }
}

pub(crate) fn run_canonical_svg_compare(
    fact: DiagramVerificationFact,
    request: CompareRequest,
) -> Result<(), XtaskError> {
    debug_assert_eq!(
        fact.render_path(),
        VerificationRenderPath::HeadlessOperationTyped
    );

    let engine = match fact.render_profile {
        RenderProfile::Standard | RenderProfile::SequenceMath => super::svg_compare_engine(),
        RenderProfile::HandDrawnSeed => {
            super::svg_compare_engine_with_site_config(serde_json::json!({ "handDrawnSeed": 1 }))
        }
        RenderProfile::GitGraphSeed => super::svg_compare_engine_with_site_config(
            serde_json::json!({ "gitGraph": { "seed": 1 } }),
        ),
        RenderProfile::Specialist => {
            return Err(XtaskError::SvgCompareFailed(format!(
                "specialist diagram {} cannot use the canonical fact runner",
                fact.diagram
            )));
        }
    };

    // Keep the toolchain read guard alive through the family compare. This preserves the global
    // toolchain -> family lock order used by the existing Sequence adapter.
    let tools_root = crate::cmd::mermaid_cli_root();
    let toolchain_read_guard = if fact.specialist == SpecialistHook::SequenceMath {
        Some(crate::cmd::acquire_upstream_svg_toolchain_read_guard(
            &tools_root,
        )?)
    } else {
        None
    };
    let sequence_math_renderer = toolchain_read_guard
        .as_ref()
        .and_then(|guard| guard.node_katex_math_renderer());

    let layout_options = super::svg_compare_layout_opts();
    let mut environment = compare_render_environment(&request);
    if let Some(renderer) = sequence_math_renderer.clone() {
        environment = environment.with_math_renderer(renderer);
    }
    let renderer = merman::render::HeadlessRenderer::new()
        .with_engine(engine.clone())
        .with_parse_options(fact.parse_policy.options())
        .with_layout_options(layout_options)
        .with_environment(environment);

    let dom_mode = request.dom_mode.as_deref().unwrap_or(fact.default_dom_mode);
    let dom_decimals = request.dom_decimals.unwrap_or(3);
    let should_report_root = fact.diagnostics == DiagnosticsPolicy::RootDelta
        && (request.report_root || matches!(dom_mode.trim(), "parity-root" | "parity_root"));
    let root_report_limit = request
        .root_report_limit
        .unwrap_or(super::DEFAULT_ROOT_DELTA_REPORT_LIMIT);
    let mut state = CanonicalCompareState::default();

    run_svg_compare(
        CompareHarnessOptions::new(CompareRunOptions {
            diagram: fact.diagram,
            out_path: request.out_path.clone(),
            filter: request.filter.as_deref(),
            check_dom: request.check_dom,
            dom_mode,
            dom_decimals,
        }),
        &mut state,
        |_, report, paths, options| {
            let _ = writeln!(report, "# {} SVG Comparison\n", fact.report_title);
            let _ = writeln!(
                report,
                "- Upstream: `{}` (pinned Mermaid baseline)",
                paths.upstream_dir.join("*.svg").display()
            );
            let _ = writeln!(report, "- Render path: `{}`", fact.render_path().label());
            let _ = writeln!(report, "- Command: `{}`", fact.command);
            let _ = writeln!(report, "- Mode: `{}`", options.dom_mode);
            let _ = writeln!(report, "- Decimals: `{}`", options.dom_decimals);
            write_verification_policy_metadata(
                report,
                &request,
                fact,
                options.dom_mode,
                should_report_root,
            );
            if fact.specialist == SpecialistHook::SequenceMath {
                let _ = writeln!(
                    report,
                    "- Math renderer: `{}`",
                    if sequence_math_renderer.is_some() {
                        "node-katex"
                    } else {
                        "none"
                    }
                );
            }
            let _ = writeln!(
                report,
                "- Root overrides: `{}`",
                if request.apply_root_overrides {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            report.push('\n');
        },
        |_, stem, _| match fact.skip_policy {
            FixtureSkipPolicy::None => None,
            FixtureSkipPolicy::UpstreamBaseline => {
                crate::cmd::upstream_svg_baseline_skip_reason(fact.diagram, stem)
                    .map(str::to_string)
            }
            FixtureSkipPolicy::UpstreamCompare => {
                crate::cmd::upstream_svg_compare_skip_reason(fact.diagram, stem).map(str::to_string)
            }
        },
        |state, input| {
            let semantic = renderer
                .prepare_semantic_sync(input.text)
                .map_err(|error| {
                    format!("parse failed for {}: {error}", input.fixture_path.display())
                })?
                .ok_or_else(|| {
                    format!("no diagram detected in {}", input.fixture_path.display())
                })?;

            let prepared = semantic.continue_layout().map_err(|error| {
                format!(
                    "layout failed for {}: {error}",
                    input.fixture_path.display()
                )
            })?;

            let diagram_id = match fact.diagram_id_policy {
                DiagramIdPolicy::SanitizedStem => super::sanitize_svg_id(input.stem),
                DiagramIdPolicy::RawStem => input.stem.to_string(),
                DiagramIdPolicy::Specialist => {
                    return Err(format!(
                        "specialist diagram-id policy reached canonical runner for {}",
                        fact.diagram
                    ));
                }
            };
            let mut svg_options = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(diagram_id),
                ..Default::default()
            };

            match fact.specialist {
                SpecialistHook::None => {}
                SpecialistHook::ClassV2Role => {
                    let is_classdiagram_v2_header =
                        merman::preprocess_diagram(input.text, engine.registry())
                            .ok()
                            .map(|preprocessed| {
                                preprocessed
                                    .code
                                    .trim_start()
                                    .starts_with("classDiagram-v2")
                            })
                            .unwrap_or(false);
                    svg_options.aria_roledescription =
                        is_classdiagram_v2_header.then(|| "classDiagram".to_string());
                }
                SpecialistHook::SequenceMath => {}
                SpecialistHook::FlowchartAdapter
                | SpecialistHook::ErAdapter
                | SpecialistHook::GanttAdapter => {
                    return Err(format!(
                        "external specialist hook reached generic canonical runner for {}",
                        fact.diagram
                    ));
                }
            }

            let local_svg = prepared.render_svg(&svg_options).map_err(|error| {
                format!(
                    "render failed for {}: {error}",
                    input.fixture_path.display()
                )
            })?;

            let mut issues = Vec::new();
            if should_report_root {
                match super::collect_root_delta(input.stem, input.upstream_svg, &local_svg) {
                    Ok(delta) => state.root_deltas.push(delta),
                    Err(error) => {
                        issues.push(format!("root parse failed for {}: {error}", input.stem));
                    }
                }
            }

            let skip_dom_compare_for_math = fact.specialist == SpecialistHook::SequenceMath
                && input.check_dom
                && input.text.contains("$$")
                && sequence_math_renderer.is_none();
            let fixture_notes = skip_dom_compare_for_math
                .then(|| {
                    format!(
                        "skipped {}: contains `$$...$$` (Node KaTeX backend unavailable)",
                        input.stem
                    )
                })
                .into_iter()
                .collect();

            let compare_dom = !skip_dom_compare_for_math;
            Ok(match fact.compare_policy {
                FixtureComparePolicy::Dom => CompareFixtureResult::Rendered {
                    local_svg,
                    compare_dom,
                    issues,
                    notes: fixture_notes,
                },
                FixtureComparePolicy::DomAndRawSvgFallback => {
                    CompareFixtureResult::RenderedWithPolicy {
                        local_svg,
                        compare_dom,
                        compare_svg_when_dom_disabled: true,
                        issues,
                        notes: fixture_notes,
                    }
                }
                FixtureComparePolicy::Specialist => {
                    return Err(format!(
                        "specialist compare policy reached canonical runner for {}",
                        fact.diagram
                    ));
                }
            })
        },
        |_, report, fixture| {
            if fact.report_policy == FixtureReportPolicy::StatusLines {
                write_fixture_status_line(report, fixture);
            }
        },
        |state, report, paths, options, failures, notes| {
            if should_report_root {
                super::write_root_deltas_report(report, &mut state.root_deltas, root_report_limit);
            }
            if fact.report_policy == FixtureReportPolicy::Summary {
                write_compare_result_section(
                    report,
                    options.check_dom,
                    failures,
                    &paths.out_svg_dir,
                );
                write_notes_section(report, notes);
            }
        },
    )
}

pub(crate) fn write_verification_policy_metadata(
    report: &mut String,
    request: &CompareRequest,
    fact: DiagramVerificationFact,
    dom_mode: &str,
    root_diagnostics_reported: bool,
) {
    let effective_dom_mode = svgdom::DomMode::parse(dom_mode);
    let normalization = if request.check_dom {
        match effective_dom_mode {
            svgdom::DomMode::Strict => "svgdom/strict",
            svgdom::DomMode::Structure => "svgdom/structure",
            svgdom::DomMode::Parity => "svgdom/parity",
            svgdom::DomMode::ParityRoot => "svgdom/parity-root",
        }
    } else {
        "disabled (DOM check not requested)"
    };
    let root_coverage = match (request.check_dom, effective_dom_mode) {
        (false, _) => "disabled (DOM check not requested)",
        (true, svgdom::DomMode::Strict) => "checked (svgdom/strict root attributes)",
        (true, svgdom::DomMode::ParityRoot) => "checked (svgdom/parity-root viewport)",
        (true, svgdom::DomMode::Structure | svgdom::DomMode::Parity) => {
            "not-checked (selected DOM mode omits root viewport)"
        }
    };
    let root_diagnostics = if root_diagnostics_reported {
        debug_assert!(fact.supports_root_report());
        "reported"
    } else if fact.supports_root_report() {
        "available-not-requested"
    } else {
        "not-supported"
    };

    let _ = writeln!(report, "- Normalization policy: `{normalization}`");
    let _ = writeln!(
        report,
        "- Accepted residual policy: `{}`",
        request.accepted_residual_policy.label()
    );
    let _ = writeln!(report, "- Root coverage: `{root_coverage}`");
    let _ = writeln!(report, "- Root-delta diagnostics: `{root_diagnostics}`");
}

fn write_fixture_status_line(report: &mut String, fixture: &CompareFixtureReportInput<'_>) {
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_svg_compare<S, Header, Skip, Render, FixtureReport, Report>(
    harness: CompareHarnessOptions<'_>,
    state: &mut S,
    mut write_header: Header,
    mut skip_fixture: Skip,
    mut render_fixture: Render,
    mut write_fixture_report: FixtureReport,
    mut write_report: Report,
) -> Result<(), XtaskError>
where
    Header: FnMut(&mut S, &mut String, &CompareRunPaths, &CompareRunOptions<'_>),
    Skip: FnMut(&mut S, &str, &CompareRunPaths) -> Option<String>,
    Render: FnMut(&mut S, &CompareFixtureInput<'_>) -> Result<CompareFixtureResult, String>,
    FixtureReport: FnMut(&mut S, &mut String, &CompareFixtureReportInput<'_>),
    Report:
        FnMut(&mut S, &mut String, &CompareRunPaths, &CompareRunOptions<'_>, &[String], &[String]),
{
    let CompareHarnessOptions {
        mut run,
        fixtures_root,
        upstream_root,
    } = harness;
    let compare_paths = crate::cmd::compare_diagram_paths_with_roots(
        run.diagram,
        run.out_path.take(),
        fixtures_root,
        upstream_root,
    );
    let fixtures_dir = compare_paths.fixtures_dir.clone();
    let upstream_dir = compare_paths.upstream_dir.clone();
    let validate_pinned_upstream = is_pinned_upstream_dir(run.diagram, &upstream_dir);
    let out_svg_dir = compare_paths.out_svg_dir.clone();
    let _upstream_family_lock = super::acquire_upstream_svg_family_lock_for_compare(
        &upstream_dir,
        validate_pinned_upstream,
    )?;
    let root_attrs_snapshot_path =
        std::env::var_os(super::ROOT_ATTRS_SNAPSHOT_PATH_ENV).map(PathBuf::from);
    let mut root_attrs_snapshot = root_attrs_snapshot_path
        .as_ref()
        .map(|_| super::RootAttrsSnapshot::default());
    let mmd_files = crate::cmd::list_mmd_fixtures_in_dir(&fixtures_dir, run.filter, true);
    if mmd_files.is_empty() {
        return Err(XtaskError::SvgCompareFailed(format!(
            "no .mmd fixtures matched under {}",
            fixtures_dir.display()
        )));
    }
    let provenance = if validate_pinned_upstream {
        Some(crate::cmd::load_upstream_svg_provenance(
            run.diagram,
            &fixtures_dir,
            &upstream_dir,
            run.filter.is_none(),
        )?)
    } else {
        None
    };

    fs::create_dir_all(&out_svg_dir).map_err(|source| XtaskError::WriteFile {
        path: out_svg_dir.display().to_string(),
        source,
    })?;

    let mode = svgdom::DomMode::parse(run.dom_mode);
    let mut report = String::new();
    write_header(state, &mut report, &compare_paths, &run);

    let mut failures: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    for mmd_path in mmd_files {
        let Some(stem) = mmd_path.file_stem().and_then(|s| s.to_str()) else {
            failures.push(format!("invalid fixture filename {}", mmd_path.display()));
            continue;
        };

        if let Some(reason) = skip_fixture(state, stem, &compare_paths) {
            notes.push(format!("skipped {stem}: {reason}"));
            continue;
        }

        let upstream_path = upstream_dir.join(format!("{stem}.svg"));
        if let Some(provenance) = &provenance
            && let Err(err) = provenance.validate_fixture(&mmd_path, &upstream_path)
        {
            failures.push(err);
            continue;
        }
        let upstream_svg = match fs::read_to_string(&upstream_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!(
                    "missing upstream svg for {stem}: {} ({err})",
                    upstream_path.display()
                ));
                continue;
            }
        };

        let text = match fs::read_to_string(&mmd_path) {
            Ok(v) => v,
            Err(err) => {
                failures.push(format!("failed to read {}: {err}", mmd_path.display()));
                continue;
            }
        };

        let local_out_path = out_svg_dir.join(format!("{stem}.svg"));
        let input = CompareFixtureInput {
            stem,
            fixture_path: &mmd_path,
            upstream_svg: &upstream_svg,
            text: &text,
            check_dom: run.check_dom,
        };

        let outcome = match render_fixture(state, &input) {
            Ok(v) => v,
            Err(err) => {
                failures.push(err);
                continue;
            }
        };

        let failure_start = failures.len();
        match outcome {
            CompareFixtureResult::Skipped { reason } => {
                notes.push(format!("skipped {stem}: {reason}"));
                continue;
            }
            CompareFixtureResult::Rendered {
                local_svg,
                compare_dom,
                issues,
                notes: fixture_notes,
            } => {
                if let Some(snapshot) = &mut root_attrs_snapshot {
                    snapshot.capture(stem, &upstream_svg, &local_svg);
                }
                write_rendered_fixture(
                    &local_out_path,
                    &local_svg,
                    &mut failures,
                    &mut notes,
                    issues,
                    fixture_notes,
                    false,
                    run.check_dom,
                    compare_dom,
                    stem,
                    &upstream_svg,
                    &upstream_path,
                    mode,
                    run.dom_decimals,
                )?;
            }
            CompareFixtureResult::RenderedWithPolicy {
                local_svg,
                compare_dom,
                compare_svg_when_dom_disabled,
                issues,
                notes: fixture_notes,
            } => {
                if let Some(snapshot) = &mut root_attrs_snapshot {
                    snapshot.capture(stem, &upstream_svg, &local_svg);
                }
                write_rendered_fixture(
                    &local_out_path,
                    &local_svg,
                    &mut failures,
                    &mut notes,
                    issues,
                    fixture_notes,
                    compare_svg_when_dom_disabled,
                    run.check_dom,
                    compare_dom,
                    stem,
                    &upstream_svg,
                    &upstream_path,
                    mode,
                    run.dom_decimals,
                )?;
            }
        }

        write_fixture_report(
            state,
            &mut report,
            &CompareFixtureReportInput {
                stem,
                fixture_path: &mmd_path,
                upstream_path: &upstream_path,
                local_out_path: &local_out_path,
                failed: failures.len() > failure_start,
            },
        );
    }

    write_report(state, &mut report, &compare_paths, &run, &failures, &notes);

    if let Some(parent) = compare_paths.out_path.parent() {
        fs::create_dir_all(parent).map_err(|source| XtaskError::WriteFile {
            path: parent.display().to_string(),
            source,
        })?;
    }
    fs::write(&compare_paths.out_path, report).map_err(|source| XtaskError::WriteFile {
        path: compare_paths.out_path.display().to_string(),
        source,
    })?;
    // The family lock guard remains in scope through this write, so the parent audit never needs
    // to reread canonical or local SVG files after the compare process releases the lock.
    if let (Some(path), Some(snapshot)) = (&root_attrs_snapshot_path, &root_attrs_snapshot) {
        snapshot.write(path)?;
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(XtaskError::SvgCompareFailed(failures.join("\n")))
    }
}

#[allow(clippy::too_many_arguments)]
fn write_rendered_fixture(
    local_out_path: &Path,
    local_svg: &str,
    failures: &mut Vec<String>,
    notes: &mut Vec<String>,
    issues: Vec<String>,
    fixture_notes: Vec<String>,
    compare_svg_when_dom_disabled: bool,
    check_dom: bool,
    compare_dom: bool,
    stem: &str,
    upstream_svg: &str,
    upstream_path: &Path,
    mode: svgdom::DomMode,
    dom_decimals: u32,
) -> Result<(), XtaskError> {
    fs::write(local_out_path, local_svg).map_err(|source| XtaskError::WriteFile {
        path: local_out_path.display().to_string(),
        source,
    })?;

    if check_dom && compare_dom {
        if let Err(err) = compare_dom_signatures(
            stem,
            upstream_svg,
            local_svg,
            upstream_path,
            local_out_path,
            mode,
            dom_decimals,
        ) {
            failures.push(err);
        }
    } else if !check_dom && compare_svg_when_dom_disabled && upstream_svg != local_svg {
        failures.push(format!("svg mismatch for {stem}"));
    }

    failures.extend(issues);
    notes.extend(fixture_notes);
    Ok(())
}

pub(crate) fn sanitize_svg_id(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "diagram".to_string()
    } else {
        out
    }
}

fn compare_dom_signatures(
    stem: &str,
    upstream_svg: &str,
    local_svg: &str,
    upstream_path: &Path,
    local_out_path: &Path,
    mode: svgdom::DomMode,
    dom_decimals: u32,
) -> Result<(), String> {
    let upstream = svgdom::dom_signature(upstream_svg, mode, dom_decimals)
        .map_err(|err| format!("upstream dom parse failed for {stem}: {err}"))?;
    let local = svgdom::dom_signature(local_svg, mode, dom_decimals)
        .map_err(|err| format!("local dom parse failed for {stem}: {err}"))?;

    if upstream != local {
        let detail = if mode == svgdom::DomMode::ParityRoot {
            let mismatch = svgdom::diagnose_parity_root_mismatch(
                upstream_svg,
                local_svg,
                &upstream,
                &local,
                dom_decimals,
            )
            .map_err(|err| format!("parity-root diagnosis failed for {stem}: {err}"))?
            .ok_or_else(|| format!("parity-root diagnosis unexpectedly matched for {stem}"))?;
            format!(" ({mismatch})")
        } else {
            svgdom::format_dom_diffs(&svgdom::dom_diffs(&upstream, &local))
                .map(|d| format!(" ({d})"))
                .unwrap_or_default()
        };
        return Err(format!(
            "dom mismatch for {stem}: upstream={} local={}{}",
            upstream_path.display(),
            local_out_path.display(),
            detail
        ));
    }

    Ok(())
}

pub(crate) fn write_compare_result_section(
    report: &mut String,
    check_dom: bool,
    failures: &[String],
    out_svg_dir: &Path,
) {
    if !check_dom {
        let _ = writeln!(
            report,
            "\n## Result\n\nDOM check disabled (`--check-dom` not set).\n\nLocal SVG outputs: `{}`\n",
            out_svg_dir.display()
        );
    } else if failures.is_empty() {
        let _ = writeln!(report, "\n## Result\n\nAll fixtures matched.\n");
    } else {
        let _ = writeln!(report, "\n## Mismatches\n");
        for failure in failures {
            let _ = writeln!(report, "- {failure}");
        }
        let _ = writeln!(report, "\nLocal SVG outputs: `{}`\n", out_svg_dir.display());
    }
}

pub(crate) fn write_notes_section(report: &mut String, notes: &[String]) {
    if notes.is_empty() {
        return;
    }

    let _ = writeln!(
        report,
        "\n## Skipped\n\nThese fixtures are intentionally skipped (feature gaps / deferred parity).\n"
    );
    for note in notes {
        let _ = writeln!(report, "- {note}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn every_canonical_family_accepts_explicit_computed_root_policy() {
        let fact = super::super::diagram_verification_fact("state")
            .copied()
            .expect("State verification fact");

        let request = CompareRequest::parse_for_fact(vec!["--no-root-overrides".to_string()], fact)
            .expect("root policy must be a shared compare option");

        assert!(!request.apply_root_overrides);
        let session = compare_render_environment(&request)
            .begin_session()
            .expect("compare environment should begin a render session");
        assert_eq!(
            session.root_viewport_override_policy(),
            merman::render::RootViewportOverridePolicy::ComputedOnly
        );
    }

    #[test]
    fn verification_metadata_reports_the_effective_comparison_policies() {
        let fact = super::super::diagram_verification_fact("flowchart")
            .copied()
            .expect("Flowchart verification fact");
        let request = CompareRequest {
            check_dom: true,
            accepted_residual_policy: AcceptedResidualPolicy::RootParityExact,
            ..CompareRequest::default()
        };
        let mut report = String::new();

        write_verification_policy_metadata(&mut report, &request, fact, "parity-root", true);

        assert!(report.contains("Normalization policy: `svgdom/parity-root`"));
        assert!(report.contains("exact fail-closed root residual registry"));
        assert!(report.contains("Root coverage: `checked (svgdom/parity-root viewport)`"));
        assert!(report.contains("Root-delta diagnostics: `reported`"));
    }

    #[test]
    fn verification_metadata_distinguishes_family_specific_dom_residual_policies() {
        let cases = [
            (
                "sequence",
                AcceptedResidualPolicy::SequenceExactDom,
                "exact fail-closed Sequence DOM residual registry",
            ),
            (
                "quadrantchart",
                AcceptedResidualPolicy::QuadrantStructureRenderability,
                "source-backed QuadrantChart renderability correction registry",
            ),
            ("info", AcceptedResidualPolicy::None, "policy: `none`"),
        ];

        for (diagram, policy, expected) in cases {
            let fact = super::super::diagram_verification_fact(diagram)
                .copied()
                .expect("verification fact should exist");
            let request = CompareRequest {
                check_dom: true,
                accepted_residual_policy: policy,
                ..CompareRequest::default()
            };
            let mut report = String::new();

            write_verification_policy_metadata(&mut report, &request, fact, "structure", false);

            assert!(
                report.contains(expected),
                "diagram={diagram}; report={report}"
            );
        }
    }

    #[test]
    fn verification_metadata_does_not_conflate_root_diagnostics_with_root_coverage() {
        let fact = super::super::diagram_verification_fact("flowchart")
            .copied()
            .expect("Flowchart verification fact");
        let request = CompareRequest {
            check_dom: true,
            ..CompareRequest::default()
        };
        let mut report = String::new();

        write_verification_policy_metadata(&mut report, &request, fact, "parity", true);

        assert!(
            report.contains("Root coverage: `not-checked (selected DOM mode omits root viewport)`")
        );
        assert!(report.contains("Root-delta diagnostics: `reported`"));
    }

    #[test]
    fn parity_root_failure_marks_normalized_descendant_match() {
        let upstream = r#"<svg width="100%" viewBox="0 0 100 100" style="max-width: 100px; background-color: white;"><g transform="translate(10,20)"/></svg>"#;
        let local = r#"<svg width="100%" viewBox="0 0 120 100" style="max-width: 120px; background-color: white;"><g transform="translate(10,20)"/></svg>"#;

        let failure = compare_dom_signatures(
            "root-only",
            upstream,
            local,
            Path::new("upstream.svg"),
            Path::new("local.svg"),
            svgdom::DomMode::ParityRoot,
            3,
        )
        .expect_err("root viewport mismatch should fail");

        assert!(failure.contains(svgdom::PARITY_NORMALIZED_DESCENDANTS_MATCH_MARKER));
        assert!(failure.contains("svg: attr `style` mismatch"));
        assert!(failure.contains(svgdom::ADDITIONAL_DOM_DIFFS_MARKER));
        assert!(failure.contains("svg: attr `viewBox` mismatch"));
        assert_eq!(failure.lines().count(), 1);
    }

    #[test]
    fn parity_root_failure_prioritizes_hidden_parity_visible_mismatch() {
        let upstream = r#"<svg width="100%" viewBox="0 0 100 100" style="max-width: 100px; background-color: white;"><g transform="translate(10,20)"/></svg>"#;
        let local = r#"<svg width="100%" viewBox="0 0 120 100" style="max-width: 120px; background-color: white;"><g transform="scale(10,20)"/></svg>"#;

        let failure = compare_dom_signatures(
            "root-and-subtree",
            upstream,
            local,
            Path::new("upstream.svg"),
            Path::new("local.svg"),
            svgdom::DomMode::ParityRoot,
            3,
        )
        .expect_err("parity-visible subtree mismatch should fail");

        assert!(failure.contains(svgdom::PARITY_NORMALIZED_DESCENDANTS_DIFFER_MARKER));
        assert!(failure.contains("root-viewport-also-differs=true"));
        assert!(failure.contains("svg/g[0]: attr `transform` mismatch"));
        assert!(!failure.contains("max-width: 100px"));
        assert_eq!(failure.lines().count(), 1);
    }

    #[test]
    fn dom_failure_reports_all_same_fixture_differences() {
        let upstream = r#"<svg data-root="upstream"><g data-node="upstream">upstream</g></svg>"#;
        let local = r#"<svg data-root="local"><g data-node="local">local</g></svg>"#;

        let failure = compare_dom_signatures(
            "multiple-differences",
            upstream,
            local,
            Path::new("upstream.svg"),
            Path::new("local.svg"),
            svgdom::DomMode::Strict,
            3,
        )
        .expect_err("three same-fixture differences should fail");

        assert!(failure.contains("svg: attr `data-root` mismatch"));
        assert!(failure.contains(svgdom::ADDITIONAL_DOM_DIFFS_MARKER));
        assert!(failure.contains("svg/g[0]: attr `data-node` mismatch"));
        assert!(failure.contains("svg/g[0]: text mismatch"));
        assert_eq!(failure.lines().count(), 1);
    }

    fn unique_test_root(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        crate::cmd::target_root()
            .join("compare")
            .join("xtask-harness-tests")
            .join(format!("{name}-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn explicit_canonical_upstream_path_still_requires_pinned_provenance() {
        let pinned = crate::cmd::fixtures_root().join("upstream-svgs").join("er");
        assert!(is_pinned_upstream_dir("er", &pinned));
        assert!(is_pinned_upstream_dir("er", &pinned.join(".")));
    }

    #[test]
    fn svg_compare_harness_supports_custom_roots_and_render_level_skips() {
        let root = unique_test_root("roots-and-skips");
        let fixtures_root = root.join("fixtures");
        let upstream_root = root.join("upstream");
        let fixture_dir = fixtures_root.join("harness_probe");
        let upstream_dir = upstream_root.join("harness_probe");
        fs::create_dir_all(&fixture_dir).expect("fixture dir should be created");
        fs::create_dir_all(&upstream_dir).expect("upstream dir should be created");
        fs::write(fixture_dir.join("rendered.mmd"), "rendered")
            .expect("rendered fixture should be written");
        fs::write(fixture_dir.join("skipped.mmd"), "skipped")
            .expect("skipped fixture should be written");
        fs::write(upstream_dir.join("rendered.svg"), r#"<svg id="rendered"/>"#)
            .expect("rendered upstream should be written");
        fs::write(upstream_dir.join("skipped.svg"), r#"<svg id="skipped"/>"#)
            .expect("skipped upstream should be written");

        let out_path = root.join("report.md");
        let mut seen = Vec::new();
        run_svg_compare(
            CompareHarnessOptions {
                run: CompareRunOptions {
                    diagram: "harness_probe",
                    out_path: Some(out_path.clone()),
                    filter: None,
                    check_dom: true,
                    dom_mode: "parity",
                    dom_decimals: 3,
                },
                fixtures_root: Some(fixtures_root),
                upstream_root: Some(upstream_root),
            },
            &mut seen,
            |_, report, _paths, _options| {
                let _ = writeln!(report, "# Harness Probe");
            },
            |_, _, _| None,
            |seen, input| {
                seen.push(input.stem.to_string());
                if input.stem == "skipped" {
                    return Ok(CompareFixtureResult::Skipped {
                        reason: "parse-time admission policy".to_string(),
                    });
                }
                Ok(CompareFixtureResult::Rendered {
                    local_svg: input.upstream_svg.to_string(),
                    compare_dom: true,
                    issues: Vec::new(),
                    notes: Vec::new(),
                })
            },
            |_, _, _| {},
            |_, report, paths, options, failures, notes| {
                write_compare_result_section(
                    report,
                    options.check_dom,
                    failures,
                    &paths.out_svg_dir,
                );
                write_notes_section(report, notes);
            },
        )
        .expect("custom-root harness run should succeed");

        assert_eq!(seen, ["rendered", "skipped"]);
        let report = fs::read_to_string(&out_path).expect("report should be written");
        assert!(report.contains("All fixtures matched."));
        assert!(report.contains("skipped skipped: parse-time admission policy"));
        let out_svg_dir = out_path
            .parent()
            .expect("out path should have parent")
            .join("harness_probe");
        assert!(out_svg_dir.join("rendered.svg").is_file());
        assert!(!out_svg_dir.join("skipped.svg").is_file());
    }

    #[test]
    fn svg_compare_harness_keeps_missing_upstream_and_render_errors_distinct() {
        let root = unique_test_root("distinct-errors");
        let fixtures_root = root.join("fixtures");
        let upstream_root = root.join("upstream");
        let fixture_dir = fixtures_root.join("harness_probe");
        let upstream_dir = upstream_root.join("harness_probe");
        fs::create_dir_all(&fixture_dir).expect("fixture dir should be created");
        fs::create_dir_all(&upstream_dir).expect("upstream dir should be created");
        fs::write(fixture_dir.join("missing.mmd"), "missing")
            .expect("missing fixture should be written");
        fs::write(fixture_dir.join("parse-error.mmd"), "parse-error")
            .expect("parse-error fixture should be written");
        fs::write(upstream_dir.join("parse-error.svg"), "<svg/>")
            .expect("parse-error upstream should be written");

        let error = run_svg_compare(
            CompareHarnessOptions {
                run: CompareRunOptions {
                    diagram: "harness_probe",
                    out_path: Some(root.join("report.md")),
                    filter: None,
                    check_dom: false,
                    dom_mode: "structure",
                    dom_decimals: 3,
                },
                fixtures_root: Some(fixtures_root),
                upstream_root: Some(upstream_root),
            },
            &mut (),
            |_, _, _, _| {},
            |_, _, _| None,
            |_, input| Err(format!("parse failed for {}", input.fixture_path.display())),
            |_, _, _| {},
            |_, report, paths, options, failures, notes| {
                write_compare_result_section(
                    report,
                    options.check_dom,
                    failures,
                    &paths.out_svg_dir,
                );
                write_notes_section(report, notes);
            },
        )
        .expect_err("both fixture errors should fail the compare");
        let message = error.to_string();

        assert!(
            message.contains("missing upstream svg for missing"),
            "{message}"
        );
        assert!(message.contains("parse failed for"), "{message}");
        assert!(message.contains("parse-error.mmd"), "{message}");
    }
}
