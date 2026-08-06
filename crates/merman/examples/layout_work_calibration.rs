use merman::svg::{
    HeadlessError, HeadlessRenderer, RenderError, RenderResourcePolicy, RenderResourceProfile,
    ResourceLimitCause, ResourceLimitExceeded, ResourceLimitId, ResourceLimitPhase,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

const LAYOUT_WORK_LIMIT_ID: &str = "max_layout_work_units";
const DEFAULT_BOUNDARY_MAX_ITERATIONS: usize = 65_536;
const ARCHITECTURE_BOUNDARY_NODES: usize = 32;
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
    boundary_max_iterations: usize,
}

#[derive(Debug, Serialize)]
struct CalibrationReport {
    schema_version: u32,
    authoritative_date: String,
    provenance: Provenance,
    policy: PolicyReport,
    fixture_corpus: FixtureCorpusReport,
    boundary: BoundaryReport,
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
    exact_limit_check: ExactLimitCheck,
    fixtures: Vec<FixtureReport>,
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
struct BoundaryReport {
    curve: &'static str,
    search_contract: &'static str,
    fixed_nodes: usize,
    fixed_edges: usize,
    minimum_effective_iterations: usize,
    first_rejected_iterations: usize,
    accepted: BoundaryAccepted,
    rejected: BoundaryRejected,
    next_rejected: BoundaryRejected,
}

#[derive(Debug, Serialize)]
struct BoundaryAccepted {
    configured_iterations: usize,
    source_sha256: String,
    source_bytes: usize,
    layout_work_units: usize,
    svg_sha256: String,
    svg_bytes: usize,
    svg_elements: usize,
}

#[derive(Debug, Serialize)]
struct BoundaryRejected {
    configured_iterations: usize,
    source_sha256: String,
    source_bytes: usize,
    rejection: LimitRejection,
}

enum BoundaryProbe {
    Accepted(BoundaryAccepted),
    Rejected(BoundaryRejected),
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

    let fixture_corpus = calibrate_fixture_corpus(
        &corpus,
        &workspace_root,
        max_layout_work_units,
        args.expected_max_fixture.as_deref(),
    )?;
    let boundary =
        find_architecture_iteration_boundary(max_layout_work_units, args.boundary_max_iterations)?;
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
        boundary,
    };

    if let Some(parent) = args.json_out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.json_out, serde_json::to_vec_pretty(&report)?)?;
    println!("wrote {}", args.json_out.display());
    Ok(())
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
    let mut boundary_max_iterations = DEFAULT_BOUNDARY_MAX_ITERATIONS;
    let mut args = env::args().skip(1);

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--authoritative-date" => authoritative_date = Some(next_value(&mut args, &argument)?),
            "--corpus" => corpus_path = PathBuf::from(next_value(&mut args, &argument)?),
            "--json-out" => json_out = Some(PathBuf::from(next_value(&mut args, &argument)?)),
            "--expected-max-fixture" => {
                expected_max_fixture = Some(next_value(&mut args, &argument)?)
            }
            "--boundary-max-iterations" => {
                boundary_max_iterations = parse_positive_usize(&mut args, &argument)?
            }
            "--help" | "-h" => {
                println!(
                    "usage: layout_work_calibration --authoritative-date YYYY-MM-DD \\
                     --json-out PATH [--corpus PATH] [--expected-max-fixture NAME] \\
                     [--boundary-max-iterations N]"
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

    Ok(Arguments {
        authoritative_date,
        corpus_path,
        json_out,
        expected_max_fixture,
        boundary_max_iterations,
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
    let interactive =
        HeadlessRenderer::new().with_resource_profile(RenderResourceProfile::Interactive);
    let unbounded = HeadlessRenderer::new()
        .with_resource_profile(RenderResourceProfile::UnboundedForTrustedInput);
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
            .render_svg_report_sync(&source)?
            .ok_or_else(|| format!("{}: unsupported fixture", input.name))?;
        let interactive_render = interactive
            .render_svg_report_sync(&source)?
            .ok_or_else(|| format!("{}: unsupported interactive fixture", input.name))?;
        if unbounded_render.svg() != interactive_render.svg() {
            return Err(format!(
                "{}: resource profile changed successful SVG output",
                input.name
            )
            .into());
        }
        if unbounded_render.report().layout_work_units()
            != interactive_render.report().layout_work_units()
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
            layout_work_units: interactive_render.report().layout_work_units(),
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
        exact_limit_check,
        fixtures,
    })
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
    let accepted = HeadlessRenderer::new()
        .with_resource_policy(accepted_policy)
        .render_svg_report_sync(source)?
        .ok_or("maximum fixture was not recognized at its exact work limit")?;
    if accepted.report().layout_work_units() != observed_layout_work_units {
        return Err(format!(
            "maximum fixture work changed at its exact limit: expected {observed_layout_work_units}, observed {}",
            accepted.report().layout_work_units()
        )
        .into());
    }
    let accepted_svg_sha256 = sha256_hex(accepted.svg().as_bytes());
    if accepted_svg_sha256 != expected.svg_sha256 {
        return Err("maximum fixture SVG changed at its exact work limit".into());
    }

    let rejected_policy = RenderResourcePolicy::interactive()
        .with_limit(ResourceLimitId::MaxLayoutWorkUnits, rejected_limit)?;
    let rejection = match HeadlessRenderer::new()
        .with_resource_policy(rejected_policy)
        .render_svg_report_sync(source)
    {
        Err(HeadlessError::Render(RenderError::ResourceLimitExceeded(limit))) => {
            validate_layout_rejection(limit, rejected_limit, Some(rejected_limit))?
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
    expected_max: usize,
    expected_override: Option<usize>,
) -> Result<LimitRejection, Box<dyn Error>> {
    if limit.cause != ResourceLimitCause::Ceiling
        || limit.phase != ResourceLimitPhase::LayoutModel
        || limit.limit != LAYOUT_WORK_LIMIT_ID
        || limit.profile != RenderResourceProfile::Interactive
        || limit.max != expected_max
        || limit.actual <= limit.max
    {
        return Err(format!(
            "unexpected layout-work rejection: cause={} phase={} limit={} actual={} max={} profile={}",
            limit.cause,
            limit.phase,
            limit.limit,
            limit.actual,
            limit.max,
            limit.profile.id()
        )
        .into());
    }
    match expected_override {
        Some(value)
            if limit.explicit_overrides.len() == 1
                && limit.explicit_overrides[0].id == ResourceLimitId::MaxLayoutWorkUnits
                && limit.explicit_overrides[0].value == value => {}
        None if limit.explicit_overrides.is_empty() => {}
        _ => return Err("layout-work rejection reported unexpected explicit overrides".into()),
    }

    Ok(LimitRejection {
        cause: limit.cause.as_str().to_string(),
        phase: limit.phase.to_string(),
        limit: limit.limit.to_string(),
        actual: limit.actual,
        max: limit.max,
        profile: limit.profile.id().to_string(),
        explicit_overrides: limit
            .explicit_overrides
            .into_iter()
            .map(|entry| PolicyOverride {
                id: entry.id.as_str(),
                value: entry.value,
            })
            .collect(),
    })
}

fn find_architecture_iteration_boundary(
    max_layout_work_units: usize,
    boundary_max_iterations: usize,
) -> Result<BoundaryReport, Box<dyn Error>> {
    let renderer =
        HeadlessRenderer::new().with_resource_profile(RenderResourceProfile::Interactive);
    let minimum_effective_iterations = ARCHITECTURE_BOUNDARY_NODES * 5;
    let mut accepted_iterations = minimum_effective_iterations;
    match probe_architecture_iterations(&renderer, accepted_iterations, max_layout_work_units)? {
        BoundaryProbe::Accepted(_) => {}
        BoundaryProbe::Rejected(_) => {
            return Err("minimum effective Architecture iteration count must be accepted".into());
        }
    }

    let mut rejected_iterations = accepted_iterations
        .checked_mul(2)
        .ok_or("Architecture iteration count overflow")?
        .min(boundary_max_iterations);
    while let BoundaryProbe::Accepted(_) =
        probe_architecture_iterations(&renderer, rejected_iterations, max_layout_work_units)?
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
        match probe_architecture_iterations(&renderer, midpoint, max_layout_work_units)? {
            BoundaryProbe::Accepted(_) => accepted_iterations = midpoint,
            BoundaryProbe::Rejected(_) => rejected_iterations = midpoint,
        }
    }

    let accepted =
        match probe_architecture_iterations(&renderer, accepted_iterations, max_layout_work_units)?
        {
            BoundaryProbe::Accepted(accepted) => accepted,
            BoundaryProbe::Rejected(_) => {
                return Err("boundary accepted point became rejected".into());
            }
        };
    let rejected =
        match probe_architecture_iterations(&renderer, rejected_iterations, max_layout_work_units)?
        {
            BoundaryProbe::Rejected(rejected) => rejected,
            BoundaryProbe::Accepted(_) => {
                return Err("boundary rejected point became accepted".into());
            }
        };
    let next_iterations = rejected_iterations
        .checked_add(1)
        .ok_or("Architecture iteration count overflow")?;
    let next_rejected =
        match probe_architecture_iterations(&renderer, next_iterations, max_layout_work_units)? {
            BoundaryProbe::Rejected(rejected) => rejected,
            BoundaryProbe::Accepted(_) => {
                return Err(
                    "Architecture iteration boundary was not monotonic after rejection".into(),
                );
            }
        };

    Ok(BoundaryReport {
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
    renderer: &HeadlessRenderer,
    configured_iterations: usize,
    max_layout_work_units: usize,
) -> Result<BoundaryProbe, Box<dyn Error>> {
    let source = architecture_iteration_source(configured_iterations);
    let source_sha256 = sha256_hex(source.as_bytes());
    let source_bytes = source.len();

    match renderer.render_svg_report_sync(&source) {
        Ok(Some(rendered)) => {
            let svg = rendered.svg();
            let document = roxmltree::Document::parse(svg)?;
            Ok(BoundaryProbe::Accepted(BoundaryAccepted {
                configured_iterations,
                source_sha256,
                source_bytes,
                layout_work_units: rendered.report().layout_work_units(),
                svg_sha256: sha256_hex(svg.as_bytes()),
                svg_bytes: svg.len(),
                svg_elements: document
                    .descendants()
                    .filter(|node| node.is_element())
                    .count(),
            }))
        }
        Ok(None) => Err("Architecture iteration probe was not recognized".into()),
        Err(HeadlessError::Render(RenderError::ResourceLimitExceeded(limit))) => {
            Ok(BoundaryProbe::Rejected(BoundaryRejected {
                configured_iterations,
                source_sha256,
                source_bytes,
                rejection: validate_layout_rejection(limit, max_layout_work_units, None)?,
            }))
        }
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
}
