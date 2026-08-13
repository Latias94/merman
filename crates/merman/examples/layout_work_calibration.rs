use merman::render::{ResourceLimitCause, ResourceLimitExceeded};
use merman::svg::{
    RenderEnvironment, RenderResourcePolicy, RenderResourceProfile, ResourceLimitId,
};
use merman::{
    OperationControl, RenderError, RenderOutput, RenderRequest, RenderTarget, Renderer,
    SemanticArtifact, SvgLayoutOutput, SvgOutput, SvgRequest,
};
use merman_core::ParseOptions;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::time::Instant;

const LAYOUT_WORK_LIMIT_ID: &str = "max_layout_work_units";
const DEFAULT_BOUNDARY_MAX_NODES: usize = 16_384;
const DEFAULT_BOUNDARY_MAX_ITERATIONS: usize = 65_536;
const ARCHITECTURE_BOUNDARY_NODES: usize = 32;
const MINIMUM_HEADROOM_UNITS: usize = 100_000;
const MINIMUM_HEADROOM_PERCENT: f64 = 10.0;
const CEILING_ROUNDING_QUANTUM: usize = 100_000;
const EXPECTED_CALIBRATION_FEATURES: [&str; 5] = [
    "svg",
    "layout-cytoscape",
    "layout-elk",
    "math",
    "complete-svg",
];

#[derive(Debug)]
struct Arguments {
    authoritative_date: String,
    corpus_path: PathBuf,
    json_out: PathBuf,
    expected_max_fixture: Option<String>,
    boundary_max_nodes: usize,
    boundary_max_iterations: usize,
    probe: Option<ProbeRequest>,
}

#[derive(Debug, Serialize)]
struct CalibrationReport {
    schema_version: u32,
    authoritative_date: String,
    provenance: Provenance,
    policy: PolicyReport,
    fixture_corpus: FixtureCorpusReport,
    cardinality_boundary: CardinalityBoundaryReport,
    configuration_boundary: ConfigurationBoundaryReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeRequest {
    input: ProbeInput,
    stage: ProbeStage,
    limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ProbeInput {
    Fixture(String),
    LinearFlowchart { nodes: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum ProbeStage {
    Semantic,
    Layout,
    Svg,
    EndToEnd,
}

#[derive(Debug, Serialize)]
struct SingleProbeReport {
    schema_version: u32,
    report_kind: &'static str,
    authoritative_date: String,
    provenance: Provenance,
    policy: PolicyReport,
    input: SingleProbeInputReport,
    stage: ProbeStage,
    outcome: SingleProbeOutcome,
}

struct SingleProbeContext<'a> {
    workspace_root: &'a Path,
    corpus: &'a CorpusSelection,
    owned_paths: &'a [PathBuf],
    snapshot: &'a SourceSnapshot,
    base_policy: RenderResourcePolicy,
    authoritative_date: String,
}

#[derive(Debug, Serialize)]
struct SingleProbeInputReport {
    kind: &'static str,
    name: String,
    source_path: Option<String>,
    nodes: Option<usize>,
    edges: Option<usize>,
    source_sha256: String,
    source_bytes: usize,
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum SingleProbeOutcome {
    AcceptedSemantic {
        elapsed_ns: u64,
        semantic_kind: String,
        diagram_type: String,
    },
    AcceptedLayout {
        elapsed_ns: u64,
        layout_json_sha256: String,
        layout_json_bytes: usize,
    },
    AcceptedSvg {
        elapsed_ns: u64,
        layout_work_units: usize,
        svg_sha256: String,
        svg_bytes: usize,
        svg_elements: usize,
    },
    Rejected {
        elapsed_ns: u64,
        rejection: LimitRejection,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceSnapshot {
    git_revision: String,
    cargo_lock_sha256: String,
    cargo_manifest_sha256: String,
    corpus_manifest_sha256: String,
    calibration_source_sha256: String,
    executable_sha256: String,
}

#[derive(Debug, Serialize)]
struct Provenance {
    git_revision: String,
    tracked_worktree_clean: bool,
    owned_inputs_tracked: bool,
    postflight_identical: bool,
    cargo_lock_sha256: String,
    cargo_manifest_sha256: String,
    corpus_manifest_sha256: String,
    calibration_source_sha256: String,
    executable_path: String,
    executable_sha256: String,
    build_profile: &'static str,
    target_os: &'static str,
    target_arch: &'static str,
    enabled_features: Vec<&'static str>,
    rustc: String,
    cargo: String,
}

#[derive(Debug, Serialize)]
struct PolicyReport {
    profile: &'static str,
    explicit_overrides: Vec<PolicyOverride>,
    max_layout_work_units: usize,
}

#[derive(Debug, Serialize)]
struct PolicyOverride {
    id: &'static str,
    value: usize,
}

#[derive(Debug, Serialize)]
struct FixtureCorpusReport {
    corpus_manifest: String,
    corpus_schema_version: u32,
    fixture_members_sha256: String,
    fixture_count: usize,
    maximum_layout_work_fixture: String,
    maximum_layout_work_units: usize,
    headroom_units: usize,
    headroom_percent: f64,
    headroom_policy: HeadroomPolicyReport,
    exact_limit_check: ExactLimitCheck,
    fixtures: Vec<FixtureReport>,
}

#[derive(Debug, Serialize)]
struct HeadroomPolicyReport {
    minimum_headroom_units: usize,
    minimum_headroom_percent: f64,
    ceiling_rounding_quantum: usize,
    calculated_minimum_ceiling: usize,
}

#[derive(Debug, Serialize)]
struct FixtureReport {
    name: String,
    source_path: String,
    source_sha256: String,
    source_bytes: usize,
    layout_work_units: usize,
    svg_sha256: String,
    svg_bytes: usize,
    svg_elements: usize,
    interactive_accepted: bool,
}

#[derive(Debug, Serialize)]
struct ExactLimitCheck {
    fixture: String,
    observed_layout_work_units: usize,
    accepted_limit: usize,
    accepted_explicit_overrides: Vec<PolicyOverride>,
    accepted_svg_sha256: String,
    rejected_limit: usize,
    rejection: LimitRejection,
}

#[derive(Debug, Serialize)]
struct LimitRejection {
    cause: String,
    phase: String,
    limit: String,
    actual: usize,
    max: usize,
    profile: String,
    explicit_overrides: Vec<PolicyOverride>,
}

#[derive(Debug, Serialize)]
struct ConfigurationBoundaryReport {
    curve: &'static str,
    search_contract: &'static str,
    fixed_nodes: usize,
    fixed_edges: usize,
    minimum_effective_iterations: usize,
    first_rejected_iterations: usize,
    accepted: ConfigurationBoundaryAccepted,
    rejected: ConfigurationBoundaryRejected,
    next_rejected: ConfigurationBoundaryRejected,
}

#[derive(Debug, Serialize)]
struct ConfigurationBoundaryAccepted {
    configured_iterations: usize,
    source_sha256: String,
    source_bytes: usize,
    layout_work_units: usize,
    svg_sha256: String,
    svg_bytes: usize,
    svg_elements: usize,
}

#[derive(Debug, Serialize)]
struct ConfigurationBoundaryRejected {
    configured_iterations: usize,
    source_sha256: String,
    source_bytes: usize,
    rejection: LimitRejection,
}

enum ConfigurationBoundaryProbe {
    Accepted(ConfigurationBoundaryAccepted),
    Rejected(ConfigurationBoundaryRejected),
}

#[derive(Debug, Serialize)]
struct CardinalityBoundaryReport {
    curve: &'static str,
    search_contract: &'static str,
    scan_start_nodes: usize,
    scanned_through_nodes: usize,
    accepted_prefix_count: usize,
    accepted_prefix_digest_encoding: &'static str,
    accepted_prefix_observations_sha256: String,
    accepted_prefix_work_units_non_decreasing: bool,
    first_rejected_nodes: usize,
    accepted: CardinalityBoundaryAccepted,
    rejected: CardinalityBoundaryRejected,
    next_rejected: CardinalityBoundaryRejected,
}

#[derive(Debug, Serialize)]
struct CardinalityBoundaryAccepted {
    nodes: usize,
    edges: usize,
    source_sha256: String,
    source_bytes: usize,
    layout_work_units: usize,
    svg_sha256: String,
    svg_bytes: usize,
    svg_elements: usize,
}

#[derive(Debug, Serialize)]
struct CardinalityBoundaryRejected {
    nodes: usize,
    edges: usize,
    source_sha256: String,
    source_bytes: usize,
    rejection: LimitRejection,
}

enum CardinalityBoundaryProbe {
    Accepted(CardinalityBoundaryAccepted),
    Rejected(CardinalityBoundaryRejected),
}

struct CardinalityScanAccepted {
    nodes: usize,
    layout_work_units: usize,
}

enum CardinalityScanProbe {
    Accepted(CardinalityScanAccepted),
    Rejected(CardinalityBoundaryRejected),
}

#[derive(Debug, Deserialize)]
struct CorpusManifest {
    schema_version: u32,
    fixtures: Vec<CorpusFixtureEntry>,
}

#[derive(Debug, Deserialize)]
struct CorpusFixtureEntry {
    name: String,
    source: String,
}

#[derive(Debug)]
struct CorpusSelection {
    path: PathBuf,
    schema_version: u32,
    manifest_sha256: String,
    fixtures: Vec<FixtureInput>,
}

#[derive(Debug)]
struct FixtureInput {
    name: String,
    source_path: String,
    path: PathBuf,
}

/// Typed SVG facade configured with one render policy for calibration probes.
///
/// The renderer owns source parsing and the operation control. The SVG request only carries
/// target-local environment/options; it never creates a replacement operation.
#[derive(Clone)]
struct CalibrationRenderer {
    renderer: Renderer,
    policy: RenderResourcePolicy,
    svg_request: SvgRequest,
}

impl CalibrationRenderer {
    fn new(policy: RenderResourcePolicy) -> Self {
        Self {
            renderer: Renderer::new()
                .with_parse_options(ParseOptions::strict())
                .with_resource_policy(policy.input_policy().clone()),
            policy,
            svg_request: SvgRequest {
                environment: RenderEnvironment::deterministic().with_resource_policy(policy),
                ..SvgRequest::default()
            },
        }
    }

    fn prepare_semantic(&self, source: &str) -> Result<Option<SemanticArtifact>, RenderError> {
        self.renderer
            .prepare_semantic(source, OperationControl::new())
    }

    fn render_svg(&self, source: &str) -> Result<Option<SvgOutput>, RenderError> {
        let output = self.renderer.render(RenderRequest::svg(
            source,
            OperationControl::new(),
            self.svg_request.clone(),
        ))?;
        let RenderOutput::Svg(output) = output else {
            unreachable!("SVG request must return SVG output")
        };
        Ok(output)
    }

    fn render_svg_from_semantic(
        &self,
        semantic: SemanticArtifact,
    ) -> Result<Option<SvgOutput>, RenderError> {
        let output =
            self.render_semantic_target(semantic, RenderTarget::Svg(self.svg_request.clone()))?;
        let RenderOutput::Svg(output) = output else {
            unreachable!("SVG target must return SVG output")
        };
        Ok(output)
    }

    fn render_layout_from_semantic(
        &self,
        semantic: SemanticArtifact,
    ) -> Result<Option<SvgLayoutOutput>, RenderError> {
        let output = self
            .render_semantic_target(semantic, RenderTarget::LayoutJson(self.svg_request.clone()))?;
        let RenderOutput::LayoutJson(output) = output else {
            unreachable!("layout target must return layout output")
        };
        Ok(output)
    }

    fn render_semantic_target(
        &self,
        semantic: SemanticArtifact,
        target: RenderTarget,
    ) -> Result<RenderOutput, RenderError> {
        semantic.render(target)
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = parse_arguments()?;
    validate_calibration_features()?;
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let corpus = load_corpus(&workspace_root, &args.corpus_path)?;
    let owned_paths = owned_input_paths(&workspace_root, &corpus);
    let preflight = capture_source_snapshot(&workspace_root, &corpus, &owned_paths)?;
    let policy = RenderResourcePolicy::interactive();
    let max_layout_work_units = policy
        .value(ResourceLimitId::MaxLayoutWorkUnits)
        .ok_or("interactive profile must bound layout work")?;

    if let Some(probe) = args.probe {
        let report = run_single_probe(
            SingleProbeContext {
                workspace_root: &workspace_root,
                corpus: &corpus,
                owned_paths: &owned_paths,
                snapshot: &preflight,
                base_policy: policy,
                authoritative_date: args.authoritative_date,
            },
            probe,
        )?;
        write_json_report(&args.json_out, &report)?;
        return Ok(());
    }

    let fixture_corpus = calibrate_fixture_corpus(
        &corpus,
        &workspace_root,
        max_layout_work_units,
        args.expected_max_fixture.as_deref(),
    )?;
    let cardinality_boundary = find_flowchart_cardinality_boundary(args.boundary_max_nodes)?;
    let configuration_boundary =
        find_architecture_iteration_boundary(args.boundary_max_iterations)?;
    let postflight = capture_source_snapshot(&workspace_root, &corpus, &owned_paths)?;
    if preflight != postflight {
        return Err("calibration inputs or executable changed during the run".into());
    }

    let report = CalibrationReport {
        schema_version: 1,
        authoritative_date: args.authoritative_date,
        provenance: provenance(&workspace_root, preflight)?,
        policy: PolicyReport {
            profile: RenderResourceProfile::Interactive.id(),
            explicit_overrides: policy
                .explicit_overrides()
                .map(|(id, value)| PolicyOverride {
                    id: id.as_str(),
                    value,
                })
                .collect(),
            max_layout_work_units,
        },
        fixture_corpus,
        cardinality_boundary,
        configuration_boundary,
    };

    write_json_report(&args.json_out, &report)?;
    Ok(())
}

fn write_json_report<T: Serialize>(path: &Path, report: &T) -> Result<(), Box<dyn Error>> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(report)?)?;
    println!("wrote {}", path.display());
    Ok(())
}

fn run_single_probe(
    context: SingleProbeContext<'_>,
    request: ProbeRequest,
) -> Result<SingleProbeReport, Box<dyn Error>> {
    let (input, source) = resolve_probe_source(context.corpus, &request.input)?;
    let policy = match request.limit {
        Some(limit) => context
            .base_policy
            .with_limit(ResourceLimitId::MaxLayoutWorkUnits, limit)?,
        None => context.base_policy,
    };
    let renderer = CalibrationRenderer::new(policy);
    let outcome = match request.stage {
        ProbeStage::Semantic => {
            let started = Instant::now();
            let semantic = renderer
                .prepare_semantic(&source)?
                .ok_or("probe source was not recognized")?;
            SingleProbeOutcome::AcceptedSemantic {
                elapsed_ns: elapsed_ns(started),
                semantic_kind: semantic.semantic_kind().to_string(),
                diagram_type: semantic.diagram_type().to_string(),
            }
        }
        ProbeStage::Layout => {
            let semantic = renderer
                .prepare_semantic(&source)?
                .ok_or("probe source was not recognized")?;
            let started = Instant::now();
            match renderer.render_layout_from_semantic(semantic) {
                Ok(Some(layout)) => {
                    let bytes = serde_json::to_vec(layout.layout())?;
                    SingleProbeOutcome::AcceptedLayout {
                        elapsed_ns: elapsed_ns(started),
                        layout_json_sha256: sha256_hex(&bytes),
                        layout_json_bytes: bytes.len(),
                    }
                }
                Ok(None) => return Err("layout probe returned no diagram".into()),
                Err(RenderError::ResourceLimitExceeded(limit)) => SingleProbeOutcome::Rejected {
                    elapsed_ns: elapsed_ns(started),
                    rejection: validate_layout_rejection(limit, policy)?,
                },
                Err(error) => {
                    return Err(format!("layout probe failed unexpectedly: {error}").into());
                }
            }
        }
        ProbeStage::Svg => {
            let semantic = renderer
                .prepare_semantic(&source)?
                .ok_or("probe source was not recognized")?;
            let started = Instant::now();
            match renderer.render_svg_from_semantic(semantic) {
                Ok(Some(rendered)) => {
                    let svg = rendered.svg();
                    let document = roxmltree::Document::parse(svg)?;
                    SingleProbeOutcome::AcceptedSvg {
                        elapsed_ns: elapsed_ns(started),
                        layout_work_units: rendered.evidence().layout_work_units(),
                        svg_sha256: sha256_hex(svg.as_bytes()),
                        svg_bytes: svg.len(),
                        svg_elements: document
                            .descendants()
                            .filter(|node| node.is_element())
                            .count(),
                    }
                }
                Ok(None) => return Err("SVG probe returned no diagram".into()),
                Err(RenderError::ResourceLimitExceeded(limit)) => SingleProbeOutcome::Rejected {
                    elapsed_ns: elapsed_ns(started),
                    rejection: validate_layout_rejection(limit, policy)?,
                },
                Err(error) => return Err(format!("SVG probe failed unexpectedly: {error}").into()),
            }
        }
        ProbeStage::EndToEnd => {
            let started = Instant::now();
            match renderer.render_svg(&source) {
                Ok(Some(rendered)) => {
                    let svg = rendered.svg();
                    let document = roxmltree::Document::parse(svg)?;
                    SingleProbeOutcome::AcceptedSvg {
                        elapsed_ns: elapsed_ns(started),
                        layout_work_units: rendered.evidence().layout_work_units(),
                        svg_sha256: sha256_hex(svg.as_bytes()),
                        svg_bytes: svg.len(),
                        svg_elements: document
                            .descendants()
                            .filter(|node| node.is_element())
                            .count(),
                    }
                }
                Ok(None) => return Err("probe source was not recognized".into()),
                Err(RenderError::ResourceLimitExceeded(limit)) => SingleProbeOutcome::Rejected {
                    elapsed_ns: elapsed_ns(started),
                    rejection: validate_layout_rejection(limit, policy)?,
                },
                Err(error) => {
                    return Err(format!("end-to-end probe failed unexpectedly: {error}").into());
                }
            }
        }
    };
    let postflight =
        capture_source_snapshot(context.workspace_root, context.corpus, context.owned_paths)?;
    if *context.snapshot != postflight {
        return Err("calibration inputs or executable changed during the probe".into());
    }

    Ok(SingleProbeReport {
        schema_version: 1,
        report_kind: "single_probe",
        authoritative_date: context.authoritative_date,
        provenance: provenance(context.workspace_root, context.snapshot.clone())?,
        policy: policy_report(&policy)?,
        input,
        stage: request.stage,
        outcome,
    })
}

fn resolve_probe_source(
    corpus: &CorpusSelection,
    input: &ProbeInput,
) -> Result<(SingleProbeInputReport, String), Box<dyn Error>> {
    match input {
        ProbeInput::Fixture(name) => {
            let fixture = corpus
                .fixtures
                .iter()
                .find(|fixture| fixture.name == *name)
                .ok_or_else(|| format!("probe fixture not found in corpus: {name}"))?;
            let source = fs::read_to_string(&fixture.path)?;
            Ok((
                SingleProbeInputReport {
                    kind: "fixture",
                    name: fixture.name.clone(),
                    source_path: Some(fixture.source_path.clone()),
                    nodes: None,
                    edges: None,
                    source_sha256: sha256_hex(source.as_bytes()),
                    source_bytes: source.len(),
                },
                source,
            ))
        }
        ProbeInput::LinearFlowchart { nodes } => {
            let source = linear_flowchart_source(*nodes);
            Ok((
                SingleProbeInputReport {
                    kind: "linear_flowchart",
                    name: format!("linear_flowchart_{nodes}"),
                    source_path: None,
                    nodes: Some(*nodes),
                    edges: Some(nodes.saturating_sub(1)),
                    source_sha256: sha256_hex(source.as_bytes()),
                    source_bytes: source.len(),
                },
                source,
            ))
        }
    }
}

fn elapsed_ns(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn policy_report(policy: &RenderResourcePolicy) -> Result<PolicyReport, Box<dyn Error>> {
    Ok(PolicyReport {
        profile: RenderResourceProfile::Interactive.id(),
        explicit_overrides: policy
            .explicit_overrides()
            .map(|(id, value)| PolicyOverride {
                id: id.as_str(),
                value,
            })
            .collect(),
        max_layout_work_units: policy
            .value(ResourceLimitId::MaxLayoutWorkUnits)
            .ok_or("interactive probe policy must bound layout work")?,
    })
}

fn parse_arguments() -> Result<Arguments, Box<dyn Error>> {
    let mut authoritative_date = None;
    let mut corpus_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tools")
        .join("bench")
        .join("corpus.json");
    let mut json_out = None;
    let mut expected_max_fixture = None;
    let mut boundary_max_nodes = DEFAULT_BOUNDARY_MAX_NODES;
    let mut boundary_max_iterations = DEFAULT_BOUNDARY_MAX_ITERATIONS;
    let mut probe_input = None;
    let mut probe_stage = None;
    let mut probe_limit = None;
    let mut args = env::args().skip(1);

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--authoritative-date" => authoritative_date = Some(next_value(&mut args, &argument)?),
            "--corpus" => corpus_path = PathBuf::from(next_value(&mut args, &argument)?),
            "--json-out" => json_out = Some(PathBuf::from(next_value(&mut args, &argument)?)),
            "--expected-max-fixture" => {
                expected_max_fixture = Some(next_value(&mut args, &argument)?)
            }
            "--boundary-max-nodes" => {
                boundary_max_nodes = parse_positive_usize(&mut args, &argument)?
            }
            "--boundary-max-iterations" => {
                boundary_max_iterations = parse_positive_usize(&mut args, &argument)?
            }
            "--probe-fixture" => {
                if probe_input.is_some() {
                    return Err("only one probe input may be selected".into());
                }
                probe_input = Some(ProbeInput::Fixture(next_value(&mut args, &argument)?));
            }
            "--probe-flowchart-nodes" => {
                if probe_input.is_some() {
                    return Err("only one probe input may be selected".into());
                }
                probe_input = Some(ProbeInput::LinearFlowchart {
                    nodes: parse_positive_usize(&mut args, &argument)?,
                });
            }
            "--probe-stage" => {
                probe_stage = Some(parse_probe_stage(&next_value(&mut args, &argument)?)?);
            }
            "--probe-limit" => {
                probe_limit = Some(parse_positive_usize(&mut args, &argument)?);
            }
            "--help" | "-h" => {
                println!(
                    "usage: layout_work_calibration --authoritative-date YYYY-MM-DD \\
                     --json-out PATH [--corpus PATH] [--expected-max-fixture NAME] \\
                     [--boundary-max-nodes N] [--boundary-max-iterations N] \\
                     [--probe-fixture NAME | --probe-flowchart-nodes N] \\
                     [--probe-stage semantic|layout|svg|end-to-end] [--probe-limit N]"
                );
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }

    let authoritative_date = authoritative_date.ok_or("--authoritative-date is required")?;
    validate_authoritative_date(&authoritative_date)?;
    let json_out = json_out.ok_or("--json-out is required")?;
    if boundary_max_iterations <= ARCHITECTURE_BOUNDARY_NODES * 5 {
        return Err(format!(
            "--boundary-max-iterations must exceed {}",
            ARCHITECTURE_BOUNDARY_NODES * 5
        )
        .into());
    }
    if boundary_max_nodes < 2 {
        return Err("--boundary-max-nodes must be at least 2".into());
    }
    let probe = match (probe_input, probe_stage, probe_limit) {
        (None, None, None) => None,
        (Some(input), Some(stage), limit) => Some(ProbeRequest {
            input,
            stage,
            limit,
        }),
        (None, Some(_), _) => return Err("--probe-stage requires a probe input".into()),
        (Some(_), None, _) => return Err("a probe input requires --probe-stage".into()),
        (None, None, Some(_)) => return Err("--probe-limit requires a probe input".into()),
    };

    Ok(Arguments {
        authoritative_date,
        corpus_path,
        json_out,
        expected_max_fixture,
        boundary_max_nodes,
        boundary_max_iterations,
        probe,
    })
}

fn next_value(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<String, Box<dyn Error>> {
    args.next()
        .ok_or_else(|| format!("{flag} requires a value").into())
}

fn parse_positive_usize(
    args: &mut impl Iterator<Item = String>,
    flag: &str,
) -> Result<usize, Box<dyn Error>> {
    let value = next_value(args, flag)?;
    let parsed = value
        .parse::<usize>()
        .map_err(|error| format!("invalid {flag} value `{value}`: {error}"))?;
    if parsed == 0 {
        return Err(format!("{flag} must be positive").into());
    }
    Ok(parsed)
}

fn parse_probe_stage(value: &str) -> Result<ProbeStage, Box<dyn Error>> {
    match value {
        "semantic" => Ok(ProbeStage::Semantic),
        "layout" => Ok(ProbeStage::Layout),
        "svg" => Ok(ProbeStage::Svg),
        "end-to-end" => Ok(ProbeStage::EndToEnd),
        _ => Err(format!("unknown --probe-stage value `{value}`").into()),
    }
}

fn validate_authoritative_date(value: &str) -> Result<(), Box<dyn Error>> {
    let bytes = value.as_bytes();
    if bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return Ok(());
    }
    Err("--authoritative-date must use YYYY-MM-DD".into())
}

fn load_corpus(
    workspace_root: &Path,
    configured_path: &Path,
) -> Result<CorpusSelection, Box<dyn Error>> {
    let path = if configured_path.is_absolute() {
        configured_path.to_path_buf()
    } else {
        workspace_root.join(configured_path)
    }
    .canonicalize()?;
    path.strip_prefix(workspace_root)
        .map_err(|_| "corpus manifest must stay inside the workspace")?;

    let bytes = fs::read(&path)?;
    let manifest: CorpusManifest = serde_json::from_slice(&bytes)?;
    if manifest.schema_version != 2 {
        return Err(format!(
            "unsupported benchmark corpus schema {}; expected 2",
            manifest.schema_version
        )
        .into());
    }
    if manifest.fixtures.is_empty() {
        return Err("benchmark corpus contains no fixtures".into());
    }

    let mut names = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut fixtures = Vec::with_capacity(manifest.fixtures.len());
    for fixture in manifest.fixtures {
        if !names.insert(fixture.name.clone()) {
            return Err(format!("duplicate corpus fixture name `{}`", fixture.name).into());
        }
        if !sources.insert(fixture.source.clone()) {
            return Err(format!("duplicate corpus fixture source `{}`", fixture.source).into());
        }
        let relative = Path::new(&fixture.source);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(format!(
                "corpus fixture source must be a normalized workspace-relative path: {}",
                fixture.source
            )
            .into());
        }
        if relative
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("mmd")
        {
            return Err(format!("corpus fixture is not an .mmd file: {}", fixture.source).into());
        }
        let source_path = workspace_root.join(relative).canonicalize()?;
        source_path
            .strip_prefix(workspace_root)
            .map_err(|_| format!("corpus fixture escapes the workspace: {}", fixture.source))?;
        fixtures.push(FixtureInput {
            name: fixture.name,
            source_path: fixture.source,
            path: source_path,
        });
    }

    Ok(CorpusSelection {
        path,
        schema_version: manifest.schema_version,
        manifest_sha256: sha256_hex(&bytes),
        fixtures,
    })
}

fn owned_input_paths(workspace_root: &Path, corpus: &CorpusSelection) -> Vec<PathBuf> {
    let mut paths = vec![
        workspace_root.join("Cargo.lock"),
        workspace_root.join("crates/merman/Cargo.toml"),
        workspace_root.join("crates/merman/examples/layout_work_calibration.rs"),
        corpus.path.clone(),
    ];
    paths.extend(corpus.fixtures.iter().map(|fixture| fixture.path.clone()));
    paths
}

fn capture_source_snapshot(
    workspace_root: &Path,
    corpus: &CorpusSelection,
    owned_paths: &[PathBuf],
) -> Result<SourceSnapshot, Box<dyn Error>> {
    let tracked_status = command_required(
        workspace_root,
        "git",
        &["status", "--porcelain", "--untracked-files=no"],
    )?;
    if !tracked_status.is_empty() {
        return Err("tracked worktree must be clean for decision-grade calibration".into());
    }
    for path in owned_paths {
        let relative = workspace_relative_path(path, workspace_root)?;
        command_required(
            workspace_root,
            "git",
            &["ls-files", "--error-unmatch", "--", &relative],
        )
        .map_err(|_| format!("calibration input is not tracked at HEAD: {relative}"))?;
    }

    let executable = env::current_exe()?.canonicalize()?;
    let corpus_manifest_sha256 = sha256_file(&corpus.path)?;
    if corpus_manifest_sha256 != corpus.manifest_sha256 {
        return Err("corpus manifest changed after it was parsed".into());
    }
    Ok(SourceSnapshot {
        git_revision: command_required(workspace_root, "git", &["rev-parse", "HEAD"])?,
        cargo_lock_sha256: sha256_file(&workspace_root.join("Cargo.lock"))?,
        cargo_manifest_sha256: sha256_file(&workspace_root.join("crates/merman/Cargo.toml"))?,
        corpus_manifest_sha256,
        calibration_source_sha256: sha256_file(
            &workspace_root.join("crates/merman/examples/layout_work_calibration.rs"),
        )?,
        executable_sha256: sha256_file(&executable)?,
    })
}

fn calibrate_fixture_corpus(
    corpus: &CorpusSelection,
    workspace_root: &Path,
    max_layout_work_units: usize,
    expected_max_fixture: Option<&str>,
) -> Result<FixtureCorpusReport, Box<dyn Error>> {
    let interactive = CalibrationRenderer::new(RenderResourcePolicy::interactive());
    let unbounded = CalibrationRenderer::new(RenderResourcePolicy::unbounded_for_trusted_input());
    let mut fixtures = Vec::with_capacity(corpus.fixtures.len());
    let mut member_hasher = Sha256::new();

    for input in &corpus.fixtures {
        let source = fs::read_to_string(&input.path)?;
        let source_sha256 = sha256_hex(source.as_bytes());
        member_hasher.update(input.name.as_bytes());
        member_hasher.update([0]);
        member_hasher.update(input.source_path.as_bytes());
        member_hasher.update([0]);
        member_hasher.update(source_sha256.as_bytes());
        member_hasher.update([b'\n']);
        let unbounded_render = unbounded
            .render_svg(&source)?
            .ok_or_else(|| format!("{}: unsupported fixture", input.name))?;
        let interactive_render = interactive
            .render_svg(&source)?
            .ok_or_else(|| format!("{}: unsupported interactive fixture", input.name))?;
        if unbounded_render.svg() != interactive_render.svg() {
            return Err(format!(
                "{}: resource profile changed successful SVG output",
                input.name
            )
            .into());
        }
        if unbounded_render.evidence().layout_work_units()
            != interactive_render.evidence().layout_work_units()
        {
            return Err(format!(
                "{}: resource profile changed successful work accounting",
                input.name
            )
            .into());
        }

        let svg = interactive_render.svg();
        let document = roxmltree::Document::parse(svg)?;
        fixtures.push(FixtureReport {
            name: input.name.clone(),
            source_path: input.source_path.clone(),
            source_sha256,
            source_bytes: source.len(),
            layout_work_units: interactive_render.evidence().layout_work_units(),
            svg_sha256: sha256_hex(svg.as_bytes()),
            svg_bytes: svg.len(),
            svg_elements: document
                .descendants()
                .filter(|node| node.is_element())
                .count(),
            interactive_accepted: true,
        });
    }

    let maximum = fixtures
        .iter()
        .max_by_key(|fixture| fixture.layout_work_units)
        .ok_or("fixture corpus unexpectedly empty")?;
    if let Some(expected) = expected_max_fixture
        && maximum.name != expected
    {
        return Err(format!(
            "maximum layout-work fixture changed: expected {expected}, observed {}",
            maximum.name
        )
        .into());
    }
    if maximum.layout_work_units > max_layout_work_units {
        return Err(format!(
            "interactive layout-work limit {max_layout_work_units} rejects maximum fixture {} at {} units",
            maximum.name, maximum.layout_work_units
        )
        .into());
    }
    let headroom_units = max_layout_work_units - maximum.layout_work_units;
    let headroom_percent = headroom_units as f64 * 100.0 / maximum.layout_work_units as f64;
    let required_with_minimum = maximum
        .layout_work_units
        .checked_add(MINIMUM_HEADROOM_UNITS)
        .ok_or("headroom policy overflow")?;
    let calculated_minimum_ceiling =
        round_up_to_quantum(required_with_minimum, CEILING_ROUNDING_QUANTUM)?;
    if max_layout_work_units != calculated_minimum_ceiling
        || headroom_units < MINIMUM_HEADROOM_UNITS
        || headroom_percent < MINIMUM_HEADROOM_PERCENT
    {
        return Err(format!(
            "interactive ceiling {max_layout_work_units} does not satisfy the registered headroom policy: minimum_units={MINIMUM_HEADROOM_UNITS}, minimum_percent={MINIMUM_HEADROOM_PERCENT}, calculated_minimum_ceiling={calculated_minimum_ceiling}, observed_headroom_units={headroom_units}, observed_headroom_percent={headroom_percent:.6}"
        )
        .into());
    }
    let maximum_input = corpus
        .fixtures
        .iter()
        .find(|input| input.name == maximum.name)
        .ok_or("maximum corpus fixture disappeared")?;
    let maximum_source = fs::read_to_string(&maximum_input.path)?;
    let exact_limit_check = verify_exact_work_limit(&maximum_source, maximum)?;

    Ok(FixtureCorpusReport {
        corpus_manifest: display_relative_path(&corpus.path, workspace_root),
        corpus_schema_version: corpus.schema_version,
        fixture_members_sha256: format!("{:x}", member_hasher.finalize()),
        fixture_count: fixtures.len(),
        maximum_layout_work_fixture: maximum.name.clone(),
        maximum_layout_work_units: maximum.layout_work_units,
        headroom_units,
        headroom_percent,
        headroom_policy: HeadroomPolicyReport {
            minimum_headroom_units: MINIMUM_HEADROOM_UNITS,
            minimum_headroom_percent: MINIMUM_HEADROOM_PERCENT,
            ceiling_rounding_quantum: CEILING_ROUNDING_QUANTUM,
            calculated_minimum_ceiling,
        },
        exact_limit_check,
        fixtures,
    })
}

fn round_up_to_quantum(value: usize, quantum: usize) -> Result<usize, Box<dyn Error>> {
    if quantum == 0 {
        return Err("headroom rounding quantum must be positive".into());
    }
    let remainder = value % quantum;
    if remainder == 0 {
        return Ok(value);
    }
    value
        .checked_add(quantum - remainder)
        .ok_or_else(|| "headroom rounding overflow".into())
}

fn verify_exact_work_limit(
    source: &str,
    expected: &FixtureReport,
) -> Result<ExactLimitCheck, Box<dyn Error>> {
    let observed_layout_work_units = expected.layout_work_units;
    let rejected_limit = observed_layout_work_units
        .checked_sub(1)
        .filter(|limit| *limit > 0)
        .ok_or("maximum fixture must consume at least two layout-work units")?;

    let accepted_policy = RenderResourcePolicy::interactive().with_limit(
        ResourceLimitId::MaxLayoutWorkUnits,
        observed_layout_work_units,
    )?;
    let accepted = CalibrationRenderer::new(accepted_policy)
        .render_svg(source)?
        .ok_or("maximum fixture was not recognized at its exact work limit")?;
    if accepted.evidence().layout_work_units() != observed_layout_work_units {
        return Err(format!(
            "maximum fixture work changed at its exact limit: expected {observed_layout_work_units}, observed {}",
            accepted.evidence().layout_work_units()
        )
        .into());
    }
    let accepted_svg_sha256 = sha256_hex(accepted.svg().as_bytes());
    if accepted_svg_sha256 != expected.svg_sha256 {
        return Err("maximum fixture SVG changed at its exact work limit".into());
    }

    let rejected_policy = RenderResourcePolicy::interactive()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, rejected_limit)?;
    let rejection = match CalibrationRenderer::new(rejected_policy).render_svg(source) {
        Err(RenderError::ResourceLimitExceeded(limit)) => {
            validate_layout_rejection(limit, rejected_policy)?
        }
        Ok(_) => return Err("maximum fixture succeeded one unit below observed work".into()),
        Err(error) => {
            return Err(format!(
                "maximum fixture failed unexpectedly one unit below observed work: {error}"
            )
            .into());
        }
    };
    Ok(ExactLimitCheck {
        fixture: expected.name.clone(),
        observed_layout_work_units,
        accepted_limit: observed_layout_work_units,
        accepted_explicit_overrides: vec![PolicyOverride {
            id: LAYOUT_WORK_LIMIT_ID,
            value: observed_layout_work_units,
        }],
        accepted_svg_sha256,
        rejected_limit,
        rejection,
    })
}

fn validate_layout_rejection(
    limit: ResourceLimitExceeded,
    policy: RenderResourcePolicy,
) -> Result<LimitRejection, Box<dyn Error>> {
    let actual = usize::try_from(limit.actual)
        .map_err(|_| "layout-work rejection actual does not fit usize")?;
    let maximum = usize::try_from(limit.maximum)
        .map_err(|_| "layout-work rejection maximum does not fit usize")?;
    let expected_maximum = policy
        .value(ResourceLimitId::MaxLayoutWorkUnits)
        .ok_or("layout-work rejection policy must define a ceiling")?;
    if limit.cause != ResourceLimitCause::Ceiling
        || limit.phase != "layout_model"
        || limit.id != LAYOUT_WORK_LIMIT_ID
        || policy.profile() != RenderResourceProfile::Interactive
        || maximum != expected_maximum
        || actual <= maximum
    {
        return Err(format!(
            "unexpected layout-work rejection: cause={} phase={} limit={} actual={} max={} profile={}",
            limit.cause,
            limit.phase,
            limit.id,
            actual,
            maximum,
            policy.profile().id()
        )
        .into());
    }

    Ok(LimitRejection {
        cause: limit.cause.as_str().to_string(),
        phase: limit.phase.to_string(),
        limit: limit.id.to_string(),
        actual,
        max: maximum,
        profile: policy.profile().id().to_string(),
        explicit_overrides: policy
            .explicit_overrides()
            .map(|(id, value)| PolicyOverride {
                id: id.as_str(),
                value,
            })
            .collect(),
    })
}

fn find_flowchart_cardinality_boundary(
    boundary_max_nodes: usize,
) -> Result<CardinalityBoundaryReport, Box<dyn Error>> {
    let renderer = CalibrationRenderer::new(RenderResourcePolicy::interactive());
    let mut accepted_nodes = 0usize;
    let mut accepted_prefix_count = 0usize;
    let mut accepted_prefix_digest = Sha256::new();
    let mut previous_work_units = None;
    let mut accepted_prefix_work_units_non_decreasing = true;
    let first_rejected = 'scan: {
        for nodes in 1..=boundary_max_nodes {
            match scan_linear_flowchart_admission(&renderer, nodes)? {
                CardinalityScanProbe::Accepted(observation) => {
                    accepted_nodes = observation.nodes;
                    accepted_prefix_count = accepted_prefix_count
                        .checked_add(1)
                        .ok_or("Flowchart accepted-prefix count overflow")?;
                    let nodes_u64 = u64::try_from(observation.nodes)
                        .map_err(|_| "Flowchart node count does not fit the scan encoding")?;
                    let work_u64 = u64::try_from(observation.layout_work_units)
                        .map_err(|_| "Flowchart work does not fit the scan encoding")?;
                    accepted_prefix_digest.update(nodes_u64.to_le_bytes());
                    accepted_prefix_digest.update(work_u64.to_le_bytes());
                    if previous_work_units
                        .is_some_and(|previous| observation.layout_work_units < previous)
                    {
                        accepted_prefix_work_units_non_decreasing = false;
                    }
                    previous_work_units = Some(observation.layout_work_units);
                }
                CardinalityScanProbe::Rejected(rejected) => break 'scan rejected,
            }
        }
        return Err(format!(
            "no Flowchart layout-work rejection found by {boundary_max_nodes} nodes"
        )
        .into());
    };
    if accepted_nodes == 0 || first_rejected.nodes != accepted_nodes + 1 {
        return Err("Flowchart sequential scan did not produce an accepted prefix".into());
    }
    let rejected_nodes = first_rejected.nodes;

    let accepted = match probe_linear_flowchart(&renderer, accepted_nodes)? {
        CardinalityBoundaryProbe::Accepted(accepted) => accepted,
        CardinalityBoundaryProbe::Rejected(_) => {
            return Err("Flowchart boundary accepted point became rejected".into());
        }
    };
    let rejected = match probe_linear_flowchart(&renderer, rejected_nodes)? {
        CardinalityBoundaryProbe::Rejected(rejected) => rejected,
        CardinalityBoundaryProbe::Accepted(_) => {
            return Err("Flowchart boundary rejected point became accepted".into());
        }
    };
    let next_nodes = rejected_nodes
        .checked_add(1)
        .ok_or("Flowchart boundary node count overflow")?;
    let next_rejected = match probe_linear_flowchart(&renderer, next_nodes)? {
        CardinalityBoundaryProbe::Rejected(rejected) => rejected,
        CardinalityBoundaryProbe::Accepted(_) => {
            return Err("Flowchart first rejection was not repeated at the following point".into());
        }
    };

    Ok(CardinalityBoundaryReport {
        curve: "flowchart-linear-chain-v1",
        search_contract: "A deterministic N-node/N-1-edge linear Flowchart chain is rendered sequentially for every N starting at one. The first structured layout-work rejection defines the boundary without assuming monotonicity; N+1 is rerun only as a local consecutive-rejection check. The receipt claims only this registered curve and does not generalize the result to every Flowchart topology.",
        scan_start_nodes: 1,
        scanned_through_nodes: rejected_nodes,
        accepted_prefix_count,
        accepted_prefix_digest_encoding: "repeated u64-le(nodes) || u64-le(layout_work_units)",
        accepted_prefix_observations_sha256: format!("{:x}", accepted_prefix_digest.finalize()),
        accepted_prefix_work_units_non_decreasing,
        first_rejected_nodes: rejected_nodes,
        accepted,
        rejected,
        next_rejected,
    })
}

fn scan_linear_flowchart_admission(
    renderer: &CalibrationRenderer,
    nodes: usize,
) -> Result<CardinalityScanProbe, Box<dyn Error>> {
    let source = linear_flowchart_source(nodes);
    match renderer.render_svg(&source) {
        Ok(Some(rendered)) => Ok(CardinalityScanProbe::Accepted(CardinalityScanAccepted {
            nodes,
            layout_work_units: rendered.evidence().layout_work_units(),
        })),
        Ok(None) => Err("linear Flowchart was not recognized during sequential scan".into()),
        Err(RenderError::ResourceLimitExceeded(limit)) => Ok(CardinalityScanProbe::Rejected(
            CardinalityBoundaryRejected {
                nodes,
                edges: nodes.saturating_sub(1),
                source_sha256: sha256_hex(source.as_bytes()),
                source_bytes: source.len(),
                rejection: validate_layout_rejection(limit, renderer.policy)?,
            },
        )),
        Err(error) => Err(format!("linear Flowchart scan failed unexpectedly: {error}").into()),
    }
}

fn probe_linear_flowchart(
    renderer: &CalibrationRenderer,
    nodes: usize,
) -> Result<CardinalityBoundaryProbe, Box<dyn Error>> {
    let source = linear_flowchart_source(nodes);
    let source_sha256 = sha256_hex(source.as_bytes());
    let source_bytes = source.len();
    let edges = nodes.saturating_sub(1);

    match renderer.render_svg(&source) {
        Ok(Some(rendered)) => {
            let svg = rendered.svg();
            let document = roxmltree::Document::parse(svg)?;
            Ok(CardinalityBoundaryProbe::Accepted(
                CardinalityBoundaryAccepted {
                    nodes,
                    edges,
                    source_sha256,
                    source_bytes,
                    layout_work_units: rendered.evidence().layout_work_units(),
                    svg_sha256: sha256_hex(svg.as_bytes()),
                    svg_bytes: svg.len(),
                    svg_elements: document
                        .descendants()
                        .filter(|node| node.is_element())
                        .count(),
                },
            ))
        }
        Ok(None) => Err("linear Flowchart was not recognized".into()),
        Err(RenderError::ResourceLimitExceeded(limit)) => Ok(CardinalityBoundaryProbe::Rejected(
            CardinalityBoundaryRejected {
                nodes,
                edges,
                source_sha256,
                source_bytes,
                rejection: validate_layout_rejection(limit, renderer.policy)?,
            },
        )),
        Err(error) => Err(format!("linear Flowchart probe failed unexpectedly: {error}").into()),
    }
}

fn linear_flowchart_source(nodes: usize) -> String {
    assert!(nodes > 0);
    let mut source = String::with_capacity(nodes * 48);
    source.push_str("flowchart LR\n");
    for node in 0..nodes {
        writeln!(&mut source, "  n{node}[\"Node {node}\"]").expect("write to String");
    }
    for node in 1..nodes {
        writeln!(&mut source, "  n{} --> n{node}", node - 1).expect("write to String");
    }
    source
}

fn find_architecture_iteration_boundary(
    boundary_max_iterations: usize,
) -> Result<ConfigurationBoundaryReport, Box<dyn Error>> {
    let renderer = CalibrationRenderer::new(RenderResourcePolicy::interactive());
    let minimum_effective_iterations = ARCHITECTURE_BOUNDARY_NODES * 5;
    let mut accepted_iterations = minimum_effective_iterations;
    match probe_architecture_iterations(&renderer, accepted_iterations)? {
        ConfigurationBoundaryProbe::Accepted(_) => {}
        ConfigurationBoundaryProbe::Rejected(_) => {
            return Err("minimum effective Architecture iteration count must be accepted".into());
        }
    }

    let mut rejected_iterations = accepted_iterations
        .checked_mul(2)
        .ok_or("Architecture iteration count overflow")?
        .min(boundary_max_iterations);
    while let ConfigurationBoundaryProbe::Accepted(_) =
        probe_architecture_iterations(&renderer, rejected_iterations)?
    {
        if rejected_iterations == boundary_max_iterations {
            return Err(format!(
                "no layout-work rejection found by {boundary_max_iterations} Architecture iterations"
            )
            .into());
        }
        accepted_iterations = rejected_iterations;
        rejected_iterations = rejected_iterations
            .checked_mul(2)
            .ok_or("Architecture iteration count overflow")?
            .min(boundary_max_iterations);
    }

    while accepted_iterations + 1 < rejected_iterations {
        let midpoint = accepted_iterations + (rejected_iterations - accepted_iterations) / 2;
        match probe_architecture_iterations(&renderer, midpoint)? {
            ConfigurationBoundaryProbe::Accepted(_) => accepted_iterations = midpoint,
            ConfigurationBoundaryProbe::Rejected(_) => rejected_iterations = midpoint,
        }
    }

    let accepted = match probe_architecture_iterations(&renderer, accepted_iterations)? {
        ConfigurationBoundaryProbe::Accepted(accepted) => accepted,
        ConfigurationBoundaryProbe::Rejected(_) => {
            return Err("boundary accepted point became rejected".into());
        }
    };
    let rejected = match probe_architecture_iterations(&renderer, rejected_iterations)? {
        ConfigurationBoundaryProbe::Rejected(rejected) => rejected,
        ConfigurationBoundaryProbe::Accepted(_) => {
            return Err("boundary rejected point became accepted".into());
        }
    };
    let next_iterations = rejected_iterations
        .checked_add(1)
        .ok_or("Architecture iteration count overflow")?;
    let next_rejected = match probe_architecture_iterations(&renderer, next_iterations)? {
        ConfigurationBoundaryProbe::Rejected(rejected) => rejected,
        ConfigurationBoundaryProbe::Accepted(_) => {
            return Err("Architecture iteration boundary was not monotonic after rejection".into());
        }
    };

    Ok(ConfigurationBoundaryReport {
        curve: "architecture-num-iter-v1",
        search_contract: "A fixed 32-node/31-edge Architecture graph varies only configured numIter. Above the 5*nodes floor, FCoSE admission is a strictly increasing positive linear function of numIter for this fixed shape; binary search is accepted only after N-1 succeeds and both N and N+1 ceiling-reject max_layout_work_units.",
        fixed_nodes: ARCHITECTURE_BOUNDARY_NODES,
        fixed_edges: ARCHITECTURE_BOUNDARY_NODES - 1,
        minimum_effective_iterations,
        first_rejected_iterations: rejected_iterations,
        accepted,
        rejected,
        next_rejected,
    })
}

fn probe_architecture_iterations(
    renderer: &CalibrationRenderer,
    configured_iterations: usize,
) -> Result<ConfigurationBoundaryProbe, Box<dyn Error>> {
    let source = architecture_iteration_source(configured_iterations);
    let source_sha256 = sha256_hex(source.as_bytes());
    let source_bytes = source.len();

    match renderer.render_svg(&source) {
        Ok(Some(rendered)) => {
            let svg = rendered.svg();
            let document = roxmltree::Document::parse(svg)?;
            Ok(ConfigurationBoundaryProbe::Accepted(
                ConfigurationBoundaryAccepted {
                    configured_iterations,
                    source_sha256,
                    source_bytes,
                    layout_work_units: rendered.evidence().layout_work_units(),
                    svg_sha256: sha256_hex(svg.as_bytes()),
                    svg_bytes: svg.len(),
                    svg_elements: document
                        .descendants()
                        .filter(|node| node.is_element())
                        .count(),
                },
            ))
        }
        Ok(None) => Err("Architecture iteration probe was not recognized".into()),
        Err(RenderError::ResourceLimitExceeded(limit)) => Ok(ConfigurationBoundaryProbe::Rejected(
            ConfigurationBoundaryRejected {
                configured_iterations,
                source_sha256,
                source_bytes,
                rejection: validate_layout_rejection(limit, renderer.policy)?,
            },
        )),
        Err(error) => {
            Err(format!("Architecture iteration probe failed unexpectedly: {error}").into())
        }
    }
}

fn architecture_iteration_source(configured_iterations: usize) -> String {
    assert!(configured_iterations > 0);
    let mut source = String::with_capacity(ARCHITECTURE_BOUNDARY_NODES * 64);
    writeln!(
        &mut source,
        "%%{{init: {{\"architecture\": {{\"numIter\": {configured_iterations}, \"randomize\": false, \"seed\": 1}}}}}}%%"
    )
    .expect("write to String");
    source.push_str("architecture-beta\n");
    for node in 0..ARCHITECTURE_BOUNDARY_NODES {
        writeln!(&mut source, "  service n{node}(server)[Node {node}]").expect("write to String");
    }
    for node in 1..ARCHITECTURE_BOUNDARY_NODES {
        writeln!(&mut source, "  n{}:R -- L:n{node}", node - 1).expect("write to String");
    }
    source
}

fn provenance(
    workspace_root: &Path,
    snapshot: SourceSnapshot,
) -> Result<Provenance, Box<dyn Error>> {
    let executable = env::current_exe()?.canonicalize()?;
    Ok(Provenance {
        git_revision: snapshot.git_revision,
        tracked_worktree_clean: true,
        owned_inputs_tracked: true,
        postflight_identical: true,
        cargo_lock_sha256: snapshot.cargo_lock_sha256,
        cargo_manifest_sha256: snapshot.cargo_manifest_sha256,
        corpus_manifest_sha256: snapshot.corpus_manifest_sha256,
        calibration_source_sha256: snapshot.calibration_source_sha256,
        executable_path: display_relative_path(&executable, workspace_root),
        executable_sha256: snapshot.executable_sha256,
        build_profile: if cfg!(debug_assertions) {
            "dev"
        } else {
            "release"
        },
        target_os: env::consts::OS,
        target_arch: env::consts::ARCH,
        enabled_features: enabled_features(),
        rustc: command_required(workspace_root, "rustc", &["--version"])?,
        cargo: command_required(workspace_root, "cargo", &["--version"])?,
    })
}

fn enabled_features() -> Vec<&'static str> {
    let mut features = Vec::new();
    if cfg!(feature = "system-clock") {
        features.push("system-clock");
    }
    if cfg!(feature = "system-timezone") {
        features.push("system-timezone");
    }
    if cfg!(feature = "system-random") {
        features.push("system-random");
    }
    if cfg!(feature = "system-timing") {
        features.push("system-timing");
    }
    if cfg!(feature = "svg") {
        features.push("svg");
    }
    if cfg!(feature = "analysis") {
        features.push("analysis");
    }
    if cfg!(feature = "editor") {
        features.push("editor");
    }
    if cfg!(feature = "ascii") {
        features.push("ascii");
    }
    if cfg!(feature = "layout-cytoscape") {
        features.push("layout-cytoscape");
    }
    if cfg!(feature = "layout-elk") {
        features.push("layout-elk");
    }
    if cfg!(feature = "math") {
        features.push("math");
    }
    if cfg!(feature = "png") {
        features.push("png");
    }
    if cfg!(feature = "jpeg") {
        features.push("jpeg");
    }
    if cfg!(feature = "pdf") {
        features.push("pdf");
    }
    if cfg!(feature = "complete-svg") {
        features.push("complete-svg");
    }
    features
}

fn validate_calibration_features() -> Result<(), Box<dyn Error>> {
    let enabled = enabled_features();
    if enabled != EXPECTED_CALIBRATION_FEATURES {
        return Err(format!(
            "layout calibration requires exactly the complete-svg feature closure; enabled={enabled:?}"
        )
        .into());
    }
    Ok(())
}

fn command_required(cwd: &Path, command: &str, args: &[&str]) -> Result<String, Box<dyn Error>> {
    let output = Command::new(command).args(args).current_dir(cwd).output()?;
    if !output.status.success() {
        return Err(format!(
            "command failed: {command} {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn workspace_relative_path(path: &Path, workspace_root: &Path) -> Result<String, Box<dyn Error>> {
    let canonical = path.canonicalize()?;
    let relative = canonical
        .strip_prefix(workspace_root)
        .map_err(|_| format!("path escapes workspace: {}", canonical.display()))?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn display_relative_path(path: &Path, workspace_root: &Path) -> String {
    path.canonicalize()
        .ok()
        .and_then(|path| {
            path.strip_prefix(workspace_root)
                .ok()
                .map(Path::to_path_buf)
        })
        .unwrap_or_else(|| path.to_path_buf())
        .to_string_lossy()
        .replace('\\', "/")
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn Error>> {
    Ok(sha256_hex(&fs::read(path)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authoritative_date_requires_iso_shape() {
        assert!(validate_authoritative_date("2026-08-07").is_ok());
        assert!(validate_authoritative_date("2026-8-7").is_err());
        assert!(validate_authoritative_date("2026/08/07").is_err());
    }

    #[test]
    fn architecture_iteration_source_has_exact_cardinality() {
        let source = architecture_iteration_source(123);

        assert!(source.contains(r#""numIter": 123"#));
        assert_eq!(source.matches("  service n").count(), 32);
        assert_eq!(source.matches(":R -- L:").count(), 31);
        assert_eq!(source, architecture_iteration_source(123));
    }

    #[test]
    fn linear_flowchart_source_has_exact_cardinality() {
        let source = linear_flowchart_source(123);

        assert_eq!(source.matches("[\"Node ").count(), 123);
        assert_eq!(source.matches(" --> ").count(), 122);
        assert_eq!(source, linear_flowchart_source(123));
    }

    #[test]
    fn headroom_rounding_is_checked_and_deterministic() {
        assert_eq!(round_up_to_quantum(800_000, 100_000).unwrap(), 800_000);
        assert_eq!(round_up_to_quantum(800_001, 100_000).unwrap(), 900_000);
        assert!(round_up_to_quantum(usize::MAX, 100_000).is_err());
    }

    #[test]
    fn probe_stage_accepts_only_registered_values() {
        assert_eq!(parse_probe_stage("semantic").unwrap(), ProbeStage::Semantic);
        assert_eq!(parse_probe_stage("layout").unwrap(), ProbeStage::Layout);
        assert_eq!(parse_probe_stage("svg").unwrap(), ProbeStage::Svg);
        assert_eq!(
            parse_probe_stage("end-to-end").unwrap(),
            ProbeStage::EndToEnd
        );
        assert!(parse_probe_stage("parse").is_err());
    }
}
