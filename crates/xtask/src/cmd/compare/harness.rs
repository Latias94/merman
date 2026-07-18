//! Shared execution harness for per-diagram SVG compare commands.

use crate::XtaskError;
use crate::svgdom;
use std::fmt::Write as _;
use std::fs;
use std::ops::AddAssign;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AcceptedResidualPolicy {
    #[default]
    None,
    QuadrantStructureRenderability,
    ExactFixtureDomEvidence,
    RootParityExact,
}

impl AcceptedResidualPolicy {
    fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::QuadrantStructureRenderability => {
                "compare-all source-backed QuadrantChart renderability correction registry"
            }
            Self::ExactFixtureDomEvidence => "exact family-scoped fixture DOM evidence catalog",
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
    #[cfg(test)]
    pub(crate) representative_source: &'static str,
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
    pub(crate) const fn render_path(self) -> merman::render::RenderExecutionPath {
        merman::render::RenderExecutionPath::HeadlessOperationTyped
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
    pub(crate) flowchart_text_measurer: Option<String>,
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
            flowchart_text_measurer: None,
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
        if matches!(fact.diagram, "ishikawa" | "venn") {
            request.accepted_residual_policy = AcceptedResidualPolicy::ExactFixtureDomEvidence;
        }
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
                "--help" | "-h" => return Err(XtaskError::Usage),
                _ => return Err(XtaskError::Usage),
            }
            i += 1;
        }
        Ok(request)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderOperationContract {
    render_path: merman::render::RenderExecutionPath,
    measurement_routes: [merman::render::TextMeasurementRoute; 4],
}

impl RenderOperationContract {
    pub(crate) fn from_environment(
        environment: &merman::render::RenderEnvironment,
    ) -> Result<Self, XtaskError> {
        let session = environment.begin_session().map_err(|error| {
            XtaskError::SvgCompareFailed(format!(
                "failed to freeze compare render operation contract: {error}"
            ))
        })?;
        Ok(Self {
            render_path: merman::render::RenderExecutionPath::HeadlessOperationTyped,
            measurement_routes: session.report().measurement_routes().clone(),
        })
    }

    fn from_report(report: &merman::render::RenderOperationReport) -> Self {
        Self {
            render_path: report.execution_path(),
            measurement_routes: report.measurement_routes().clone(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ObservedRenderOperations {
    expected: RenderOperationContract,
    observed: bool,
}

#[derive(Debug)]
pub(crate) struct ObservedRenderEvidence {
    _private: (),
}

impl ObservedRenderEvidence {
    const fn render_count(self) -> usize {
        match self {
            Self { _private: () } => 1,
        }
    }
}

impl ObservedRenderOperations {
    pub(crate) fn from_environment(
        environment: &merman::render::RenderEnvironment,
    ) -> Result<Self, XtaskError> {
        Ok(Self {
            expected: RenderOperationContract::from_environment(environment)?,
            observed: false,
        })
    }

    pub(crate) fn observe(
        &mut self,
        fixture: &str,
        report: &merman::render::RenderOperationReport,
    ) -> Result<ObservedRenderEvidence, String> {
        let observed = RenderOperationContract::from_report(report);
        validate_measurement_provenance(fixture, report)?;
        if observed != self.expected {
            return Err(format!(
                "render operation contract diverged for {fixture}: expected {:?}, observed {observed:?}",
                self.expected
            ));
        }
        self.observed = true;
        Ok(ObservedRenderEvidence { _private: () })
    }

    pub(crate) fn write_report(&self, report: &mut String) {
        if !self.observed {
            let _ = writeln!(report, "- Render operation: `not-observed`");
            return;
        }
        let operation = &self.expected;

        let _ = writeln!(
            report,
            "- Render operation: `{}` (observed)",
            operation.render_path.as_str()
        );
        let _ = writeln!(
            report,
            "- Text measurement routes: `{}` (observed)",
            operation.measurement_routes.len()
        );
        for route in &operation.measurement_routes {
            let fallback = route
                .fallback
                .as_ref()
                .map(format_measurement_identity)
                .unwrap_or_else(|| "none".to_string());
            let _ = writeln!(
                report,
                "  - `{:?}`: `{:?}` primary=`{}` fallback=`{fallback}`",
                route.phase,
                route.primary_source,
                format_measurement_identity(&route.primary),
            );
        }
    }

    pub(crate) const fn has_observation(&self) -> bool {
        self.observed
    }
}

fn format_measurement_identity(
    identity: &merman::render::TextMeasurementProfileIdentity,
) -> String {
    format!("{}@{}", identity.profile(), identity.version())
}

fn validate_measurement_provenance(
    fixture: &str,
    report: &merman::render::RenderOperationReport,
) -> Result<(), String> {
    for summary in report.measurement().entries() {
        let provenance = summary.provenance();
        let Some(route) = report
            .measurement_routes()
            .iter()
            .find(|route| route.phase == provenance.phase)
        else {
            return Err(format!(
                "render operation report for {fixture} recorded {:?} without a configured measurement route",
                provenance.phase
            ));
        };
        let route_matches = match provenance.fallback_reason {
            None => {
                provenance.source == route.primary_source && provenance.identity == route.primary
            }
            Some(_) => {
                provenance.source == merman::render::TextMeasurementSource::Profile
                    && route.fallback.as_ref() == Some(&provenance.identity)
            }
        };
        if !route_matches {
            return Err(format!(
                "render operation report for {fixture} has measurement provenance outside its configured route: {provenance:?} vs {route:?}"
            ));
        }
    }
    Ok(())
}

struct CanonicalCompareState {
    root_deltas: Vec<super::RootDelta>,
    observed_operations: ObservedRenderOperations,
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompareEvidence {
    selected_fixtures: usize,
    rendered_fixtures: usize,
    skipped_fixtures: usize,
    dom_comparisons: usize,
    raw_svg_comparisons: usize,
}

impl CompareEvidence {
    pub(crate) const fn comparisons(self) -> usize {
        self.dom_comparisons + self.raw_svg_comparisons
    }

    pub(crate) fn gate_failures(self, subject: &str, check_dom: bool) -> Vec<String> {
        let mut failures = Vec::new();
        if self.rendered_fixtures == 0 {
            failures.push(format!(
                "no canonical typed render evidence for {subject}: selected={} skipped={}",
                self.selected_fixtures, self.skipped_fixtures
            ));
        }
        if check_dom && self.comparisons() == 0 {
            failures.push(format!(
                "--check-dom produced no DOM or raw SVG comparison evidence for {subject}: rendered={} skipped={}",
                self.rendered_fixtures, self.skipped_fixtures
            ));
        }
        failures
    }

    fn write_report(self, report: &mut String) {
        let _ = writeln!(
            report,
            "- Evidence counts: selected=`{}` rendered=`{}` skipped=`{}` DOM-comparisons=`{}` raw-SVG-comparisons=`{}`",
            self.selected_fixtures,
            self.rendered_fixtures,
            self.skipped_fixtures,
            self.dom_comparisons,
            self.raw_svg_comparisons,
        );
    }

    fn record_comparison(&mut self, comparison: FixtureComparison) {
        match comparison {
            FixtureComparison::None => {}
            FixtureComparison::Dom => self.dom_comparisons += 1,
            FixtureComparison::RawSvg => self.raw_svg_comparisons += 1,
        }
    }
}

impl AddAssign for CompareEvidence {
    fn add_assign(&mut self, rhs: Self) {
        self.selected_fixtures += rhs.selected_fixtures;
        self.rendered_fixtures += rhs.rendered_fixtures;
        self.skipped_fixtures += rhs.skipped_fixtures;
        self.dom_comparisons += rhs.dom_comparisons;
        self.raw_svg_comparisons += rhs.raw_svg_comparisons;
    }
}

#[derive(Debug)]
pub(crate) struct CompareRunFailure {
    evidence: CompareEvidence,
    error: Box<XtaskError>,
}

impl CompareRunFailure {
    fn with_evidence(evidence: CompareEvidence, error: XtaskError) -> Self {
        Self {
            evidence,
            error: Box::new(error),
        }
    }

    pub(crate) fn without_evidence(error: XtaskError) -> Self {
        Self::with_evidence(CompareEvidence::default(), error)
    }

    pub(crate) const fn evidence(&self) -> CompareEvidence {
        self.evidence
    }

    pub(crate) fn into_error(self) -> XtaskError {
        *self.error
    }
}

impl std::fmt::Display for CompareRunFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.as_ref().fmt(formatter)
    }
}

pub(crate) type CompareRunResult = Result<CompareEvidence, CompareRunFailure>;

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

#[derive(Debug)]
pub(crate) enum CompareFixtureResult {
    Skipped {
        reason: String,
    },
    Rendered {
        render_evidence: ObservedRenderEvidence,
        local_svg: String,
        compare_dom: bool,
        issues: Vec<String>,
        notes: Vec<String>,
    },
    RenderedWithPolicy {
        render_evidence: ObservedRenderEvidence,
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
) -> CompareRunResult {
    debug_assert_eq!(
        fact.render_path(),
        merman::render::RenderExecutionPath::HeadlessOperationTyped
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
            return Err(CompareRunFailure::without_evidence(
                XtaskError::SvgCompareFailed(format!(
                    "specialist diagram {} cannot use the canonical fact runner",
                    fact.diagram
                )),
            ));
        }
    };

    // Keep the toolchain read guard alive through the family compare. This preserves the global
    // toolchain -> family lock order used by the existing Sequence adapter.
    let tools_root = crate::cmd::mermaid_cli_root();
    let toolchain_read_guard = if fact.specialist == SpecialistHook::SequenceMath {
        Some(
            crate::cmd::acquire_upstream_svg_toolchain_read_guard(&tools_root)
                .map_err(CompareRunFailure::without_evidence)?,
        )
    } else {
        None
    };
    let sequence_math_renderer = toolchain_read_guard
        .as_ref()
        .and_then(|guard| guard.node_katex_math_renderer());

    let layout_options = super::svg_compare_layout_opts();
    let mut environment = merman::render::RenderEnvironment::parity();
    if let Some(renderer) = sequence_math_renderer.clone() {
        environment = environment.with_math_renderer(renderer);
    }
    let observed_operations = ObservedRenderOperations::from_environment(&environment)
        .map_err(CompareRunFailure::without_evidence)?;
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
    let mut state = CanonicalCompareState {
        root_deltas: Vec::new(),
        observed_operations,
    };

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
            let svg_options = merman_render::svg::SvgRenderOptions {
                diagram_id: Some(diagram_id),
                ..Default::default()
            };

            match fact.specialist {
                SpecialistHook::None => {}
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

            let rendered = prepared.render_svg_report(&svg_options).map_err(|error| {
                format!(
                    "render failed for {}: {error}",
                    input.fixture_path.display()
                )
            })?;
            let render_evidence = state
                .observed_operations
                .observe(input.stem, rendered.report())?;
            let local_svg = rendered.into_svg();

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
                    render_evidence,
                    local_svg,
                    compare_dom,
                    issues,
                    notes: fixture_notes,
                },
                FixtureComparePolicy::DomAndRawSvgFallback => {
                    CompareFixtureResult::RenderedWithPolicy {
                        render_evidence,
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
            state.observed_operations.write_report(report);
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
) -> CompareRunResult
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
    )
    .map_err(CompareRunFailure::without_evidence)?;
    let mmd_files = crate::cmd::list_mmd_fixtures_in_dir(&fixtures_dir, run.filter, true);
    if mmd_files.is_empty() {
        return Err(CompareRunFailure::without_evidence(
            XtaskError::SvgCompareFailed(format!(
                "no .mmd fixtures matched under {}",
                fixtures_dir.display()
            )),
        ));
    }
    let mut evidence = CompareEvidence {
        selected_fixtures: mmd_files.len(),
        ..CompareEvidence::default()
    };
    let provenance = if validate_pinned_upstream {
        Some(
            crate::cmd::load_upstream_svg_provenance(
                run.diagram,
                &fixtures_dir,
                &upstream_dir,
                run.filter.is_none(),
            )
            .map_err(|error| CompareRunFailure::with_evidence(evidence, error))?,
        )
    } else {
        None
    };

    fs::create_dir_all(&out_svg_dir)
        .map_err(|source| XtaskError::WriteFile {
            path: out_svg_dir.display().to_string(),
            source,
        })
        .map_err(|error| CompareRunFailure::with_evidence(evidence, error))?;

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
            evidence.skipped_fixtures += 1;
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
                evidence.skipped_fixtures += 1;
                notes.push(format!("skipped {stem}: {reason}"));
                continue;
            }
            CompareFixtureResult::Rendered {
                render_evidence,
                local_svg,
                compare_dom,
                issues,
                notes: fixture_notes,
            } => {
                evidence.rendered_fixtures += render_evidence.render_count();
                let comparison = write_rendered_fixture(
                    &local_out_path,
                    &local_svg,
                    &mut failures,
                    &mut notes,
                    issues,
                    fixture_notes,
                    false,
                    run.check_dom,
                    compare_dom,
                    run.diagram,
                    stem,
                    &upstream_svg,
                    &upstream_path,
                    mode,
                    run.dom_decimals,
                )
                .map_err(|error| CompareRunFailure::with_evidence(evidence, error))?;
                evidence.record_comparison(comparison);
            }
            CompareFixtureResult::RenderedWithPolicy {
                render_evidence,
                local_svg,
                compare_dom,
                compare_svg_when_dom_disabled,
                issues,
                notes: fixture_notes,
            } => {
                evidence.rendered_fixtures += render_evidence.render_count();
                let comparison = write_rendered_fixture(
                    &local_out_path,
                    &local_svg,
                    &mut failures,
                    &mut notes,
                    issues,
                    fixture_notes,
                    compare_svg_when_dom_disabled,
                    run.check_dom,
                    compare_dom,
                    run.diagram,
                    stem,
                    &upstream_svg,
                    &upstream_path,
                    mode,
                    run.dom_decimals,
                )
                .map_err(|error| CompareRunFailure::with_evidence(evidence, error))?;
                evidence.record_comparison(comparison);
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

    failures.extend(evidence.gate_failures(run.diagram, run.check_dom));
    evidence.write_report(&mut report);
    write_report(state, &mut report, &compare_paths, &run, &failures, &notes);

    if let Some(parent) = compare_paths.out_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|source| XtaskError::WriteFile {
                path: parent.display().to_string(),
                source,
            })
            .map_err(|error| CompareRunFailure::with_evidence(evidence, error))?;
    }
    fs::write(&compare_paths.out_path, report)
        .map_err(|source| XtaskError::WriteFile {
            path: compare_paths.out_path.display().to_string(),
            source,
        })
        .map_err(|error| CompareRunFailure::with_evidence(evidence, error))?;
    if failures.is_empty() {
        Ok(evidence)
    } else {
        Err(CompareRunFailure::with_evidence(
            evidence,
            XtaskError::SvgCompareFailed(failures.join("\n")),
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FixtureComparison {
    None,
    Dom,
    RawSvg,
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
    diagram: &str,
    stem: &str,
    upstream_svg: &str,
    upstream_path: &Path,
    mode: svgdom::DomMode,
    dom_decimals: u32,
) -> Result<FixtureComparison, XtaskError> {
    fs::write(local_out_path, local_svg).map_err(|source| XtaskError::WriteFile {
        path: local_out_path.display().to_string(),
        source,
    })?;

    if check_dom && compare_dom {
        let (profile, residual_note) = fixture_dom_profile(diagram, stem, mode);
        if let Some(reason) = residual_note {
            notes.push(format!(
                "DOM evidence for {diagram}/{stem}: requested parity attributes are bounded to structure ({reason})"
            ));
        }
        if let Err(err) = compare_dom_signatures(
            stem,
            upstream_svg,
            local_svg,
            upstream_path,
            local_out_path,
            profile,
            dom_decimals,
        ) {
            failures.push(err);
        }
        failures.extend(issues);
        notes.extend(fixture_notes);
        return Ok(FixtureComparison::Dom);
    } else if !check_dom && compare_svg_when_dom_disabled && upstream_svg != local_svg {
        failures.push(format!("svg mismatch for {stem}"));
    }

    failures.extend(issues);
    notes.extend(fixture_notes);
    Ok(if !check_dom && compare_svg_when_dom_disabled {
        FixtureComparison::RawSvg
    } else {
        FixtureComparison::None
    })
}

pub(crate) fn fixture_dom_profile(
    diagram: &str,
    stem: &str,
    requested: svgdom::DomMode,
) -> (svgdom::DomComparisonProfile, Option<&'static str>) {
    match merman_fixture_render_context::fixture_dom_evidence(diagram, stem) {
        Some(merman_fixture_render_context::FixtureDomEvidence::StructureOnly)
            if matches!(
                requested,
                svgdom::DomMode::Strict | svgdom::DomMode::Parity | svgdom::DomMode::ParityRoot
            ) =>
        {
            let profile = if requested == svgdom::DomMode::ParityRoot {
                svgdom::DomComparisonProfile::with_root_viewport(svgdom::DomMode::Structure)
            } else {
                svgdom::DomComparisonProfile::from_mode(svgdom::DomMode::Structure)
            };
            (
                profile,
                Some(merman_fixture_render_context::FixtureDomEvidence::StructureOnly.reason()),
            )
        }
        Some(merman_fixture_render_context::FixtureDomEvidence::StructureOnly) | None => {
            (svgdom::DomComparisonProfile::from_mode(requested), None)
        }
    }
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
    profile: svgdom::DomComparisonProfile,
    dom_decimals: u32,
) -> Result<(), String> {
    let upstream = svgdom::dom_signature_for_comparison(upstream_svg, profile, dom_decimals)
        .map_err(|err| format!("upstream dom parse failed for {stem}: {err}"))?;
    let local = svgdom::dom_signature_for_comparison(local_svg, profile, dom_decimals)
        .map_err(|err| format!("local dom parse failed for {stem}: {err}"))?;

    if upstream != local {
        let detail = if profile.compares_root_viewport() {
            let mismatch = svgdom::diagnose_root_viewport_mismatch(
                upstream_svg,
                local_svg,
                &upstream,
                &local,
                profile,
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
    fn every_admitted_render_family_emits_a_computed_root_viewport() {
        for fact in super::super::DIAGRAM_VERIFICATION_FACTS {
            let environment = merman::render::RenderEnvironment::parity();
            let mut observed = ObservedRenderOperations::from_environment(&environment)
                .expect("representative operation contract");
            let diagram_id = format!("computed-root-{}", fact.diagram);
            let rendered = merman::render::HeadlessRenderer::new()
                .with_engine(super::super::svg_compare_engine())
                .with_parse_options(fact.parse_policy.options())
                .with_layout_options(super::super::svg_compare_layout_opts())
                .with_environment(environment)
                .with_diagram_id(&diagram_id)
                .render_svg_report_sync(fact.representative_source)
                .unwrap_or_else(|error| {
                    panic!("{} representative render failed: {error}", fact.diagram)
                })
                .unwrap_or_else(|| {
                    panic!("{} representative source was not detected", fact.diagram)
                });
            observed
                .observe(fact.diagram, rendered.report())
                .unwrap_or_else(|error| panic!("{}: {error}", fact.diagram));

            let document = roxmltree::Document::parse(rendered.svg()).unwrap_or_else(|error| {
                panic!("{} emitted invalid root SVG: {error}", fact.diagram)
            });
            let root = document.root_element();
            assert!(root.has_tag_name("svg"), "{} root is not svg", fact.diagram);
            assert_eq!(
                root.attribute("id"),
                Some(diagram_id.as_str()),
                "{} bypassed the shared root diagram id",
                fact.diagram
            );
            let has_viewport = root.attribute("viewBox").is_some()
                || root.attribute("width").is_some()
                || root
                    .attribute("style")
                    .is_some_and(|style| style.contains("max-width:"));
            assert!(
                has_viewport,
                "{} emitted a root without a viewport",
                fact.diagram
            );
        }
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
            ("sequence", AcceptedResidualPolicy::None, "policy: `none`"),
            (
                "quadrantchart",
                AcceptedResidualPolicy::QuadrantStructureRenderability,
                "source-backed QuadrantChart renderability correction registry",
            ),
            (
                "ishikawa",
                AcceptedResidualPolicy::ExactFixtureDomEvidence,
                "exact family-scoped fixture DOM evidence catalog",
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
    fn rough_fixture_dom_profile_keeps_root_coverage_and_narrows_descendants() {
        let (profile, note) = fixture_dom_profile(
            "ishikawa",
            "upstream_cypress_ishikawa_spec_6_should_render_with_handdrawn_look_006",
            svgdom::DomMode::Parity,
        );
        assert_eq!(profile.descendants(), svgdom::DomMode::Structure);
        assert!(!profile.compares_root_viewport());
        assert!(note.expect("rough residual note").contains("RoughJS"));

        let (profile, note) =
            fixture_dom_profile("ishikawa", "new_handdrawn_fixture", svgdom::DomMode::Parity);
        assert_eq!(profile.descendants(), svgdom::DomMode::Parity);
        assert!(!profile.compares_root_viewport());
        assert_eq!(note, None);

        let (profile, note) = fixture_dom_profile(
            "ishikawa",
            "upstream_cypress_ishikawa_spec_6_should_render_with_handdrawn_look_006",
            svgdom::DomMode::ParityRoot,
        );
        assert_eq!(profile.descendants(), svgdom::DomMode::Structure);
        assert!(profile.compares_root_viewport());
        assert!(note.expect("rough residual note").contains("RoughJS"));
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
            svgdom::DomComparisonProfile::from_mode(svgdom::DomMode::ParityRoot),
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
            svgdom::DomComparisonProfile::from_mode(svgdom::DomMode::ParityRoot),
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
            svgdom::DomComparisonProfile::from_mode(svgdom::DomMode::Strict),
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

    fn render_info_for_evidence(stem: &str) -> merman::render::RenderedSvg {
        merman::render::HeadlessRenderer::new()
            .with_engine(super::super::svg_compare_engine())
            .with_layout_options(super::super::svg_compare_layout_opts())
            .with_diagram_id(stem)
            .render_svg_report_sync("info")
            .expect("Info render should succeed")
            .expect("Info should be detected")
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
        let evidence = run_svg_compare(
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
                    render_evidence: ObservedRenderEvidence { _private: () },
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
        assert_eq!(
            evidence,
            CompareEvidence {
                selected_fixtures: 2,
                rendered_fixtures: 1,
                skipped_fixtures: 1,
                dom_comparisons: 1,
                raw_svg_comparisons: 0,
            }
        );
        let report = fs::read_to_string(&out_path).expect("report should be written");
        assert!(report.contains("All fixtures matched."));
        assert!(report.contains(
            "Evidence counts: selected=`2` rendered=`1` skipped=`1` DOM-comparisons=`1` raw-SVG-comparisons=`0`"
        ));
        assert!(report.contains("skipped skipped: parse-time admission policy"));
        let out_svg_dir = out_path
            .parent()
            .expect("out path should have parent")
            .join("harness_probe");
        assert!(out_svg_dir.join("rendered.svg").is_file());
        assert!(!out_svg_dir.join("skipped.svg").is_file());
    }

    #[test]
    fn svg_output_failure_retains_the_completed_render_evidence() {
        let root = unique_test_root("svg-write-evidence");
        let fixtures_root = root.join("fixtures");
        let upstream_root = root.join("upstream");
        let fixture_dir = fixtures_root.join("harness_probe");
        let upstream_dir = upstream_root.join("harness_probe");
        fs::create_dir_all(&fixture_dir).expect("fixture dir should be created");
        fs::create_dir_all(&upstream_dir).expect("upstream dir should be created");
        fs::write(fixture_dir.join("rendered.mmd"), "info").expect("fixture should be written");
        fs::write(
            upstream_dir.join("rendered.svg"),
            render_info_for_evidence("rendered").svg(),
        )
        .expect("upstream SVG should be written");

        let out_path = root.join("report.md");
        fs::create_dir_all(root.join("harness_probe").join("rendered.svg"))
            .expect("conflicting local SVG directory should be created");
        let environment = merman::render::RenderEnvironment::parity();
        let mut observed = ObservedRenderOperations::from_environment(&environment)
            .expect("render operation contract");
        let failure = run_svg_compare(
            CompareHarnessOptions {
                run: CompareRunOptions {
                    diagram: "harness_probe",
                    out_path: Some(out_path),
                    filter: None,
                    check_dom: true,
                    dom_mode: "parity",
                    dom_decimals: 3,
                },
                fixtures_root: Some(fixtures_root),
                upstream_root: Some(upstream_root),
            },
            &mut observed,
            |_, _, _, _| {},
            |_, _, _| None,
            |observed, input| {
                let rendered = render_info_for_evidence(input.stem);
                let render_evidence = observed.observe(input.stem, rendered.report())?;
                Ok(CompareFixtureResult::Rendered {
                    render_evidence,
                    local_svg: rendered.into_svg(),
                    compare_dom: true,
                    issues: Vec::new(),
                    notes: Vec::new(),
                })
            },
            |_, _, _| {},
            |_, _, _, _, _, _| {},
        )
        .expect_err("a directory at the local SVG path should fail the write");

        assert_eq!(
            failure.evidence(),
            CompareEvidence {
                selected_fixtures: 1,
                rendered_fixtures: 1,
                skipped_fixtures: 0,
                dom_comparisons: 0,
                raw_svg_comparisons: 0,
            }
        );
        assert!(matches!(failure.into_error(), XtaskError::WriteFile { .. }));
    }

    #[test]
    fn report_output_failure_retains_completed_dom_evidence() {
        let root = unique_test_root("report-write-evidence");
        let fixtures_root = root.join("fixtures");
        let upstream_root = root.join("upstream");
        let fixture_dir = fixtures_root.join("harness_probe");
        let upstream_dir = upstream_root.join("harness_probe");
        fs::create_dir_all(&fixture_dir).expect("fixture dir should be created");
        fs::create_dir_all(&upstream_dir).expect("upstream dir should be created");
        fs::write(fixture_dir.join("rendered.mmd"), "info").expect("fixture should be written");
        fs::write(
            upstream_dir.join("rendered.svg"),
            render_info_for_evidence("rendered").svg(),
        )
        .expect("upstream SVG should be written");

        let out_path = root.join("report.md");
        fs::create_dir_all(&out_path).expect("conflicting report directory should be created");
        let environment = merman::render::RenderEnvironment::parity();
        let mut observed = ObservedRenderOperations::from_environment(&environment)
            .expect("render operation contract");
        let failure = run_svg_compare(
            CompareHarnessOptions {
                run: CompareRunOptions {
                    diagram: "harness_probe",
                    out_path: Some(out_path),
                    filter: None,
                    check_dom: true,
                    dom_mode: "parity",
                    dom_decimals: 3,
                },
                fixtures_root: Some(fixtures_root),
                upstream_root: Some(upstream_root),
            },
            &mut observed,
            |_, _, _, _| {},
            |_, _, _| None,
            |observed, input| {
                let rendered = render_info_for_evidence(input.stem);
                let render_evidence = observed.observe(input.stem, rendered.report())?;
                Ok(CompareFixtureResult::Rendered {
                    render_evidence,
                    local_svg: rendered.into_svg(),
                    compare_dom: true,
                    issues: Vec::new(),
                    notes: Vec::new(),
                })
            },
            |_, _, _| {},
            |_, _, _, _, _, _| {},
        )
        .expect_err("a directory at the report path should fail the write");

        assert_eq!(
            failure.evidence(),
            CompareEvidence {
                selected_fixtures: 1,
                rendered_fixtures: 1,
                skipped_fixtures: 0,
                dom_comparisons: 1,
                raw_svg_comparisons: 0,
            }
        );
        assert!(matches!(failure.into_error(), XtaskError::WriteFile { .. }));
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
