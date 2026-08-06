use merman::svg::{
    HeadlessError, HeadlessRenderer, RenderError, RenderResourcePolicy, RenderResourceProfile,
    ResourceLimitId,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const LAYOUT_WORK_LIMIT_ID: &str = "max_layout_work_units";
const DEFAULT_BOUNDARY_MAX_NODES: usize = 16_384;

#[derive(Debug)]
struct Arguments {
    authoritative_date: String,
    fixtures_dir: PathBuf,
    json_out: PathBuf,
    expected_max_fixture: Option<String>,
    boundary_max_nodes: usize,
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

#[derive(Debug, Serialize)]
struct Provenance {
    git_revision: Option<String>,
    tracked_worktree_dirty: Option<bool>,
    cargo_lock_sha256: Option<String>,
    build_profile: &'static str,
    target_os: &'static str,
    target_arch: &'static str,
    rustc: Option<String>,
    cargo: Option<String>,
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
    fixtures_dir: String,
    fixture_count: usize,
    maximum_layout_work_fixture: String,
    maximum_layout_work_units: usize,
    headroom_units: usize,
    headroom_percent: f64,
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
struct BoundaryReport {
    curve: &'static str,
    search_contract: &'static str,
    first_rejected_nodes: usize,
    accepted: BoundaryAccepted,
    rejected: BoundaryRejected,
    next_rejected: BoundaryRejected,
}

#[derive(Debug, Serialize)]
struct BoundaryAccepted {
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
struct BoundaryRejected {
    nodes: usize,
    edges: usize,
    source_sha256: String,
    source_bytes: usize,
    cause: String,
    phase: String,
    limit: String,
    actual: usize,
    max: usize,
    profile: String,
}

enum BoundaryProbe {
    Accepted(BoundaryAccepted),
    Rejected(BoundaryRejected),
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = parse_arguments()?;
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let policy = RenderResourcePolicy::interactive();
    let max_layout_work_units = policy
        .value(ResourceLimitId::MaxLayoutWorkUnits)
        .ok_or("interactive profile must bound layout work")?;

    let fixture_corpus = calibrate_fixture_corpus(
        &args.fixtures_dir,
        &workspace_root,
        max_layout_work_units,
        args.expected_max_fixture.as_deref(),
    )?;
    let boundary = find_flowchart_boundary(max_layout_work_units, args.boundary_max_nodes)?;

    let report = CalibrationReport {
        schema_version: 1,
        authoritative_date: args.authoritative_date,
        provenance: provenance(&workspace_root),
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
    let mut fixtures_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("fixtures");
    let mut json_out = None;
    let mut expected_max_fixture = None;
    let mut boundary_max_nodes = DEFAULT_BOUNDARY_MAX_NODES;
    let mut args = env::args().skip(1);

    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--authoritative-date" => authoritative_date = Some(next_value(&mut args, &argument)?),
            "--fixtures-dir" => fixtures_dir = PathBuf::from(next_value(&mut args, &argument)?),
            "--json-out" => json_out = Some(PathBuf::from(next_value(&mut args, &argument)?)),
            "--expected-max-fixture" => {
                expected_max_fixture = Some(next_value(&mut args, &argument)?)
            }
            "--boundary-max-nodes" => {
                boundary_max_nodes = parse_positive_usize(&mut args, &argument)?
            }
            "--help" | "-h" => {
                println!(
                    "usage: layout_work_calibration --authoritative-date YYYY-MM-DD \\
                     --json-out PATH [--fixtures-dir PATH] [--expected-max-fixture NAME] \\
                     [--boundary-max-nodes N]"
                );
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }

    let authoritative_date = authoritative_date.ok_or("--authoritative-date is required")?;
    validate_authoritative_date(&authoritative_date)?;
    let json_out = json_out.ok_or("--json-out is required")?;
    if boundary_max_nodes < 2 {
        return Err("--boundary-max-nodes must be at least 2".into());
    }

    Ok(Arguments {
        authoritative_date,
        fixtures_dir,
        json_out,
        expected_max_fixture,
        boundary_max_nodes,
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

fn calibrate_fixture_corpus(
    fixtures_dir: &Path,
    workspace_root: &Path,
    max_layout_work_units: usize,
    expected_max_fixture: Option<&str>,
) -> Result<FixtureCorpusReport, Box<dyn Error>> {
    let mut paths = fs::read_dir(fixtures_dir)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.retain(|path| path.extension().and_then(|extension| extension.to_str()) == Some("mmd"));
    paths.sort();
    if paths.is_empty() {
        return Err(format!("no .mmd fixtures under {}", fixtures_dir.display()).into());
    }

    let interactive =
        HeadlessRenderer::new().with_resource_profile(RenderResourceProfile::Interactive);
    let unbounded = HeadlessRenderer::new()
        .with_resource_profile(RenderResourceProfile::UnboundedForTrustedInput);
    let mut fixtures = Vec::with_capacity(paths.len());

    for path in paths {
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .ok_or_else(|| format!("invalid fixture name: {}", path.display()))?
            .to_string();
        let source = fs::read_to_string(&path)?;
        let unbounded_render = unbounded
            .render_svg_report_sync(&source)?
            .ok_or_else(|| format!("{name}: unsupported fixture"))?;
        let interactive_render = interactive
            .render_svg_report_sync(&source)?
            .ok_or_else(|| format!("{name}: unsupported interactive fixture"))?;
        if unbounded_render.svg() != interactive_render.svg() {
            return Err(format!("{name}: resource profile changed successful SVG output").into());
        }
        if unbounded_render.report().layout_work_units()
            != interactive_render.report().layout_work_units()
        {
            return Err(
                format!("{name}: resource profile changed successful work accounting").into(),
            );
        }

        let svg = interactive_render.svg();
        let document = roxmltree::Document::parse(svg)?;
        fixtures.push(FixtureReport {
            name,
            source_path: display_relative_path(&path, workspace_root),
            source_sha256: sha256_hex(source.as_bytes()),
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

    Ok(FixtureCorpusReport {
        fixtures_dir: display_relative_path(fixtures_dir, workspace_root),
        fixture_count: fixtures.len(),
        maximum_layout_work_fixture: maximum.name.clone(),
        maximum_layout_work_units: maximum.layout_work_units,
        headroom_units,
        headroom_percent,
        fixtures,
    })
}

fn find_flowchart_boundary(
    max_layout_work_units: usize,
    boundary_max_nodes: usize,
) -> Result<BoundaryReport, Box<dyn Error>> {
    let renderer =
        HeadlessRenderer::new().with_resource_profile(RenderResourceProfile::Interactive);
    let mut accepted_nodes = 1usize;
    match probe_linear_flowchart(&renderer, accepted_nodes)? {
        BoundaryProbe::Accepted(_) => {}
        BoundaryProbe::Rejected(_) => return Err("single-node flowchart must be accepted".into()),
    }

    let mut rejected_nodes = 2usize;
    while let BoundaryProbe::Accepted(_) = probe_linear_flowchart(&renderer, rejected_nodes)? {
        accepted_nodes = rejected_nodes;
        rejected_nodes = rejected_nodes
            .checked_mul(2)
            .ok_or("boundary node count overflow")?;
        if rejected_nodes > boundary_max_nodes {
            return Err(
                format!("no layout-work rejection found by {boundary_max_nodes} nodes").into(),
            );
        }
    }

    while accepted_nodes + 1 < rejected_nodes {
        let midpoint = accepted_nodes + (rejected_nodes - accepted_nodes) / 2;
        match probe_linear_flowchart(&renderer, midpoint)? {
            BoundaryProbe::Accepted(_) => accepted_nodes = midpoint,
            BoundaryProbe::Rejected(_) => rejected_nodes = midpoint,
        }
    }

    let accepted = match probe_linear_flowchart(&renderer, accepted_nodes)? {
        BoundaryProbe::Accepted(accepted) => accepted,
        BoundaryProbe::Rejected(_) => return Err("boundary accepted point became rejected".into()),
    };
    let rejected = match probe_linear_flowchart(&renderer, rejected_nodes)? {
        BoundaryProbe::Rejected(rejected) => rejected,
        BoundaryProbe::Accepted(_) => return Err("boundary rejected point became accepted".into()),
    };
    let next_nodes = rejected_nodes
        .checked_add(1)
        .ok_or("boundary node count overflow")?;
    let next_rejected = match probe_linear_flowchart(&renderer, next_nodes)? {
        BoundaryProbe::Rejected(rejected) => rejected,
        BoundaryProbe::Accepted(_) => {
            return Err("linear boundary was not monotonic immediately after rejection".into());
        }
    };
    if rejected.max != max_layout_work_units {
        return Err(format!(
            "boundary rejection reported max {}, expected {max_layout_work_units}",
            rejected.max
        )
        .into());
    }

    Ok(BoundaryReport {
        curve: "flowchart-linear-chain-v1",
        search_contract: "N labeled nodes and N-1 directed chain edges; binary search is accepted only after N-1 succeeds and both N and N+1 reject max_layout_work_units",
        first_rejected_nodes: rejected_nodes,
        accepted,
        rejected,
        next_rejected,
    })
}

fn probe_linear_flowchart(
    renderer: &HeadlessRenderer,
    nodes: usize,
) -> Result<BoundaryProbe, Box<dyn Error>> {
    let source = linear_flowchart_source(nodes);
    let source_sha256 = sha256_hex(source.as_bytes());
    let source_bytes = source.len();
    let edges = nodes.saturating_sub(1);

    match renderer.render_svg_report_sync(&source) {
        Ok(Some(rendered)) => {
            let svg = rendered.svg();
            let document = roxmltree::Document::parse(svg)?;
            Ok(BoundaryProbe::Accepted(BoundaryAccepted {
                nodes,
                edges,
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
        Ok(None) => Err("linear Flowchart was not recognized".into()),
        Err(HeadlessError::Render(RenderError::ResourceLimitExceeded(limit)))
            if limit.limit == LAYOUT_WORK_LIMIT_ID =>
        {
            Ok(BoundaryProbe::Rejected(BoundaryRejected {
                nodes,
                edges,
                source_sha256,
                source_bytes,
                cause: limit.cause.as_str().to_string(),
                phase: limit.phase.to_string(),
                limit: limit.limit.to_string(),
                actual: limit.actual,
                max: limit.max,
                profile: limit.profile.id().to_string(),
            }))
        }
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

fn provenance(workspace_root: &Path) -> Provenance {
    Provenance {
        git_revision: command_output(workspace_root, "git", &["rev-parse", "HEAD"]),
        tracked_worktree_dirty: command_output(
            workspace_root,
            "git",
            &["status", "--porcelain", "--untracked-files=no"],
        )
        .map(|output| !output.is_empty()),
        cargo_lock_sha256: fs::read(workspace_root.join("Cargo.lock"))
            .ok()
            .map(|bytes| sha256_hex(&bytes)),
        build_profile: if cfg!(debug_assertions) {
            "dev"
        } else {
            "release"
        },
        target_os: env::consts::OS,
        target_arch: env::consts::ARCH,
        rustc: command_output(workspace_root, "rustc", &["--version"]),
        cargo: command_output(workspace_root, "cargo", &["--version"]),
    }
}

fn command_output(cwd: &Path, command: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(command)
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout)
        .ok()
        .map(|output| output.trim().to_string())
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
    fn linear_flowchart_source_has_exact_cardinality() {
        let source = linear_flowchart_source(4);

        assert_eq!(source.matches("[\"Node ").count(), 4);
        assert_eq!(source.matches(" --> ").count(), 3);
        assert_eq!(source, linear_flowchart_source(4));
    }
}
