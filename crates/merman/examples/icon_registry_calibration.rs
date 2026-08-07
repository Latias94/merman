#![allow(unsafe_code)]

#[path = "../benches/native_memory/allocator.rs"]
mod allocator;

use allocator::{AllocationMetrics, CountingSystemAllocator};
use merman::svg::{
    HeadlessRenderer, IconPack, IconRegistry, IconRegistryResourceLimitId, RenderEnvironment,
    icon_registry_resource_limit_descriptors,
};
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fmt::Write as _;
use std::fs;
use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[global_allocator]
static ALLOCATOR: CountingSystemAllocator = CountingSystemAllocator::new();

#[derive(Debug)]
struct Arguments {
    iterations: usize,
    render_repetitions: usize,
    curated_icons_per_pack: usize,
    pack_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
struct Provenance {
    collected_at_unix_seconds: u64,
    git_revision: Option<String>,
    tracked_worktree_dirty: Option<bool>,
    cargo_lock_sha256: Option<String>,
    build_profile: &'static str,
    debug_assertions: bool,
    target_os: &'static str,
    target_arch: &'static str,
    rustc: Option<String>,
    cargo: Option<String>,
    kernel: Option<String>,
    os_version: Option<String>,
    cpu: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct PackStatistics {
    source_label: String,
    sha256: String,
    input_bytes: usize,
    prefix: String,
    last_modified: Option<u64>,
    json_members: usize,
    max_json_depth: usize,
    max_json_key_bytes: usize,
    icons: usize,
    aliases: usize,
    retained_body_bytes: usize,
    max_body_bytes: usize,
    max_xml_elements: usize,
    max_xml_depth: usize,
    max_alias_depth: usize,
    max_alias_fanout: usize,
    largest_icon_key: String,
}

#[derive(Debug, Clone, Serialize)]
struct AggregateStatistics {
    packs: usize,
    input_bytes: usize,
    json_members: usize,
    icons: usize,
    aliases: usize,
    total_entries: usize,
    retained_body_bytes: usize,
    max_json_depth: usize,
    max_json_key_bytes: usize,
    max_body_bytes: usize,
    max_xml_elements: usize,
    max_xml_depth: usize,
    max_alias_depth: usize,
    max_alias_fanout: usize,
}

#[derive(Debug, Clone, Serialize)]
struct AllocationReport {
    snapshot_live_bytes: u64,
    allocation_count: u64,
    allocated_bytes: u64,
    peak_live_bytes: u64,
    live_bytes_after: u64,
    peak_growth_bytes: u64,
    retained_growth_bytes: u64,
    post_drop_growth_bytes: u64,
    counter_overflowed: bool,
    counter_underflowed: bool,
}

#[derive(Debug, Clone, Serialize)]
struct ConstructorReport {
    cold_latency_ns: u128,
    warm_iterations: usize,
    warm_median_latency_ns: u128,
    warm_min_latency_ns: u128,
    warm_max_latency_ns: u128,
    registry_entries: usize,
    allocation: AllocationReport,
}

#[derive(Debug, Clone, Serialize)]
struct RenderReport {
    icon_key: String,
    icon_source_bytes: usize,
    repetitions: usize,
    latency_ns: u128,
    svg_bytes: usize,
    output_to_repeated_body_ratio_milli: u128,
    allocation: AllocationReport,
}

#[derive(Debug)]
struct SyntheticFixture {
    id: &'static str,
    purpose: &'static str,
    bytes: Vec<u8>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let arguments = parse_arguments()?;
    let source_labels = arguments
        .pack_paths
        .iter()
        .map(|path| portable_path_label(path))
        .collect::<Vec<_>>();
    let pack_bytes = arguments
        .pack_paths
        .iter()
        .map(fs::read)
        .collect::<Result<Vec<_>, _>>()?;
    let pack_statistics = source_labels
        .iter()
        .zip(&pack_bytes)
        .map(|(label, bytes)| analyze_pack(label, bytes))
        .collect::<Result<Vec<_>, _>>()?;
    let aggregate = aggregate_statistics(&pack_statistics);

    let real_constructor = measure_constructor(&pack_bytes, arguments.iterations)?;
    let curated_pack_bytes = pack_bytes
        .iter()
        .map(|bytes| curated_subset_pack(bytes, arguments.curated_icons_per_pack))
        .collect::<Result<Vec<_>, _>>()?;
    let curated_statistics = source_labels
        .iter()
        .zip(&curated_pack_bytes)
        .map(|(label, bytes)| analyze_pack(&format!("curated/{label}"), bytes))
        .collect::<Result<Vec<_>, _>>()?;
    let curated_aggregate = aggregate_statistics(&curated_statistics);
    let curated_constructor = measure_constructor(&curated_pack_bytes, arguments.iterations)?;

    let synthetic_fixtures = synthetic_fixtures()?;
    let synthetic_iterations = arguments.iterations.min(3);
    let synthetic_reports = synthetic_fixtures
        .iter()
        .map(|fixture| {
            let statistics = analyze_pack(fixture.id, &fixture.bytes)?;
            let constructor =
                measure_constructor(std::slice::from_ref(&fixture.bytes), synthetic_iterations)?;
            Ok::<_, Box<dyn Error>>(json!({
                "id": fixture.id,
                "purpose": fixture.purpose,
                "statistics": statistics,
                "constructor": constructor,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let largest = pack_statistics
        .iter()
        .max_by_key(|statistics| statistics.max_body_bytes)
        .ok_or("at least one pack is required")?;
    let registry = build_registry(&pack_bytes)?;
    let real_render = measure_render(
        registry,
        &largest.largest_icon_key,
        largest.max_body_bytes,
        arguments.render_repetitions,
    )?;
    let max_body_fixture = synthetic_fixtures
        .iter()
        .find(|fixture| fixture.id == "synthetic/max-body-alias-graph")
        .ok_or("the max-body synthetic fixture is missing")?;
    let synthetic_render = measure_render(
        build_registry(std::slice::from_ref(&max_body_fixture.bytes))?,
        "calibration:max-body",
        fixed_limit(IconRegistryResourceLimitId::MaxBodyBytes)?,
        arguments.render_repetitions,
    )?;

    let fixed_limits = icon_registry_resource_limit_descriptors()
        .iter()
        .map(|descriptor| {
            json!({
                "id": descriptor.stable_id,
                "phase": descriptor.phase,
                "unit": descriptor.unit,
                "description": descriptor.description,
                "value": descriptor.default_value,
                "caller_configurable": descriptor.caller_configurable,
            })
        })
        .collect::<Vec<_>>();

    let report = json!({
        "schema_version": 2,
        "package_version": env!("CARGO_PKG_VERSION"),
        "provenance": provenance(),
        "measurement_policy": {
            "real_and_curated_constructor_iterations": arguments.iterations,
            "synthetic_constructor_iterations": synthetic_iterations,
            "render_repetitions": arguments.render_repetitions,
            "curated_icons_per_pack": arguments.curated_icons_per_pack,
            "acquisition_io_in_scope": false,
            "allocator_boundary": "Rust global allocations in the measured constructor or render call; process RSS and third-party native allocator internals are outside this counter",
        },
        "fixed_constructor_limits": fixed_limits,
        "complete_collections": {
            "packs": pack_statistics,
            "aggregate": aggregate,
            "constructor": real_constructor,
        },
        "curated_subsets": {
            "packs": curated_statistics,
            "aggregate": curated_aggregate,
            "constructor": curated_constructor,
        },
        "synthetic_cases": synthetic_reports,
        "render_amplification": [real_render, synthetic_render],
    });
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

fn parse_arguments() -> Result<Arguments, Box<dyn Error>> {
    let mut iterations = 9usize;
    let mut render_repetitions = 8usize;
    let mut curated_icons_per_pack = 256usize;
    let mut pack_paths = Vec::new();
    let mut arguments = env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--iterations" => {
                iterations = arguments
                    .next()
                    .ok_or("--iterations requires a value")?
                    .parse()?;
            }
            "--render-repetitions" => {
                render_repetitions = arguments
                    .next()
                    .ok_or("--render-repetitions requires a value")?
                    .parse()?;
            }
            "--curated-icons-per-pack" => {
                curated_icons_per_pack = arguments
                    .next()
                    .ok_or("--curated-icons-per-pack requires a value")?
                    .parse()?;
            }
            "--help" | "-h" => {
                return Err(
                    "usage: icon_registry_calibration [--iterations N] [--render-repetitions N] [--curated-icons-per-pack N] PACK.json..."
                        .into(),
                );
            }
            _ if argument.starts_with('-') => {
                return Err(format!("unknown option {argument:?}").into());
            }
            _ => pack_paths.push(argument.into()),
        }
    }
    if iterations == 0
        || render_repetitions == 0
        || curated_icons_per_pack == 0
        || pack_paths.is_empty()
    {
        return Err(
            "iterations, render repetitions, curated icon count, and pack paths must be non-zero"
                .into(),
        );
    }
    Ok(Arguments {
        iterations,
        render_repetitions,
        curated_icons_per_pack,
        pack_paths,
    })
}

fn portable_path_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

fn provenance() -> Provenance {
    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let repository_root_text = repository_root.to_string_lossy().into_owned();
    let tracked_worktree_dirty = command_output(
        "git",
        &[
            "-C",
            &repository_root_text,
            "status",
            "--porcelain",
            "--untracked-files=no",
        ],
    )
    .map(|output| !output.is_empty());
    let cargo_lock_sha256 = fs::read(repository_root.join("Cargo.lock"))
        .ok()
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)));

    Provenance {
        collected_at_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs()),
        git_revision: command_output("git", &["-C", &repository_root_text, "rev-parse", "HEAD"]),
        tracked_worktree_dirty,
        cargo_lock_sha256,
        build_profile: if cfg!(debug_assertions) {
            "dev"
        } else {
            "release"
        },
        debug_assertions: cfg!(debug_assertions),
        target_os: env::consts::OS,
        target_arch: env::consts::ARCH,
        rustc: command_output("rustc", &["-Vv"]),
        cargo: command_output("cargo", &["-V"]),
        kernel: command_output("uname", &["-srv"]),
        os_version: command_output("sw_vers", &["-productVersion"]),
        cpu: command_output("sysctl", &["-n", "machdep.cpu.brand_string"])
            .or_else(|| linux_cpu_model()),
    }
}

fn command_output(program: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(program).args(arguments).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn linux_cpu_model() -> Option<String> {
    let cpuinfo = fs::read_to_string("/proc/cpuinfo").ok()?;
    cpuinfo.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == "model name").then(|| value.trim().to_owned())
    })
}

fn curated_subset_pack(bytes: &[u8], icon_limit: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let value: Value = serde_json::from_slice(bytes)?;
    let object = value
        .as_object()
        .ok_or("Iconify pack root must be an object")?;
    let icons = object
        .get("icons")
        .and_then(Value::as_object)
        .ok_or("Iconify pack icons must be an object")?;
    let selected_icons = icons
        .iter()
        .take(icon_limit)
        .map(|(name, icon)| (name.clone(), icon.clone()))
        .collect::<Map<_, _>>();
    if selected_icons.is_empty() {
        return Err("curated subset source has no direct icons".into());
    }

    let mut subset = Map::new();
    for key in ["prefix", "lastModified", "left", "top", "width", "height"] {
        if let Some(value) = object.get(key) {
            subset.insert(key.to_owned(), value.clone());
        }
    }
    subset.insert("icons".to_owned(), Value::Object(selected_icons));
    Ok(serde_json::to_vec(&Value::Object(subset))?)
}

fn synthetic_fixtures() -> Result<Vec<SyntheticFixture>, Box<dyn Error>> {
    Ok(vec![
        SyntheticFixture {
            id: "synthetic/max-body-alias-graph",
            purpose: "Exercise the exact body, XML depth, alias depth, and alias fan-out ceilings in one admitted pack",
            bytes: synthetic_body_alias_pack()?,
        },
        SyntheticFixture {
            id: "synthetic/max-entry-edge-graph",
            purpose: "Exercise the exact direct-icon, alias, total-entry, and alias-edge ceilings with one-to-one parents",
            bytes: synthetic_entry_edge_pack()?,
        },
        SyntheticFixture {
            id: "synthetic/max-xml-rewrite-plan",
            purpose: "Exercise the exact XML-element and ID-rewrite ceilings in one body without duplicate IDs",
            bytes: synthetic_xml_rewrite_pack()?,
        },
    ])
}

fn build_registry(packs: &[Vec<u8>]) -> Result<IconRegistry, Box<dyn Error>> {
    Ok(IconRegistry::from_packs(
        packs.iter().map(|bytes| IconPack::new(bytes.as_slice())),
    )?)
}

fn measure_constructor(
    packs: &[Vec<u8>],
    iterations: usize,
) -> Result<ConstructorReport, Box<dyn Error>> {
    let snapshot = ALLOCATOR.begin_measurement();
    let started = Instant::now();
    let cold_registry = match build_registry(packs) {
        Ok(registry) => registry,
        Err(error) => {
            ALLOCATOR.stop_measurement();
            return Err(error);
        }
    };
    let cold_latency_ns = started.elapsed().as_nanos();
    let allocation = ALLOCATOR.finish_measurement(snapshot);
    let registry_entries = cold_registry.len();
    black_box(&cold_registry);
    drop(cold_registry);
    let after_drop = ALLOCATOR.begin_measurement();
    ALLOCATOR.stop_measurement();

    let mut warm_latencies = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        let registry = build_registry(packs)?;
        warm_latencies.push(started.elapsed().as_nanos());
        black_box(&registry);
        drop(registry);
    }
    warm_latencies.sort_unstable();

    Ok(ConstructorReport {
        cold_latency_ns,
        warm_iterations: iterations,
        warm_median_latency_ns: warm_latencies[warm_latencies.len() / 2],
        warm_min_latency_ns: warm_latencies[0],
        warm_max_latency_ns: *warm_latencies.last().expect("non-empty timings"),
        registry_entries,
        allocation: allocation_report(allocation, after_drop),
    })
}

fn measure_render(
    registry: IconRegistry,
    icon_key: &str,
    icon_source_bytes: usize,
    repetitions: usize,
) -> Result<RenderReport, Box<dyn Error>> {
    let mut source = String::from("flowchart TD\n");
    for index in 0..repetitions {
        source.push_str(&format!(
            "N{index}@{{ icon: \"{icon_key}\", label: \"{index}\" }}\n"
        ));
    }
    let renderer = HeadlessRenderer::new()
        .with_environment(RenderEnvironment::deterministic().with_icon_registry(registry));

    let snapshot = ALLOCATOR.begin_measurement();
    let started = Instant::now();
    let svg = match renderer.render_svg_sync(&source) {
        Ok(Some(svg)) => svg,
        Ok(None) => {
            ALLOCATOR.stop_measurement();
            return Err("calibration flowchart was not detected".into());
        }
        Err(error) => {
            ALLOCATOR.stop_measurement();
            return Err(error.into());
        }
    };
    let latency_ns = started.elapsed().as_nanos();
    let allocation = ALLOCATOR.finish_measurement(snapshot);
    let svg_bytes = svg.len();
    black_box(&svg);
    drop(svg);
    let after_drop = ALLOCATOR.begin_measurement();
    ALLOCATOR.stop_measurement();
    let repeated_body_bytes = icon_source_bytes
        .checked_mul(repetitions)
        .ok_or("render amplification denominator overflowed")?;

    Ok(RenderReport {
        icon_key: icon_key.to_owned(),
        icon_source_bytes,
        repetitions,
        latency_ns,
        svg_bytes,
        output_to_repeated_body_ratio_milli: (svg_bytes as u128)
            .saturating_mul(1_000)
            .checked_div(repeated_body_bytes as u128)
            .unwrap_or_default(),
        allocation: allocation_report(allocation, after_drop),
    })
}

fn allocation_report(metrics: AllocationMetrics, after_drop: u64) -> AllocationReport {
    AllocationReport {
        snapshot_live_bytes: metrics.snapshot_live_bytes,
        allocation_count: metrics.allocation_count,
        allocated_bytes: metrics.allocated_bytes,
        peak_live_bytes: metrics.peak_live_bytes,
        live_bytes_after: metrics.live_bytes_after,
        peak_growth_bytes: metrics.peak_growth_bytes,
        retained_growth_bytes: metrics
            .live_bytes_after
            .saturating_sub(metrics.snapshot_live_bytes),
        post_drop_growth_bytes: after_drop.saturating_sub(metrics.snapshot_live_bytes),
        counter_overflowed: metrics.counter_overflowed,
        counter_underflowed: metrics.counter_underflowed,
    }
}

fn analyze_pack(source_label: &str, bytes: &[u8]) -> Result<PackStatistics, Box<dyn Error>> {
    let value: Value = serde_json::from_slice(bytes)?;
    let object = value
        .as_object()
        .ok_or("Iconify pack root must be an object")?;
    let prefix = object
        .get("prefix")
        .and_then(Value::as_str)
        .ok_or("Iconify pack prefix must be a string")?
        .to_owned();
    let icons = object
        .get("icons")
        .and_then(Value::as_object)
        .ok_or("Iconify pack icons must be an object")?;
    let aliases = object
        .get("aliases")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let mut json_members = 0usize;
    let mut max_json_depth = 0usize;
    let mut max_json_key_bytes = 0usize;
    visit_json(
        &value,
        1,
        &mut json_members,
        &mut max_json_depth,
        &mut max_json_key_bytes,
    );

    let mut retained_body_bytes = 0usize;
    let mut max_body_bytes = 0usize;
    let mut max_xml_elements = 0usize;
    let mut max_xml_depth = 0usize;
    let mut largest_icon_name = String::new();
    for (name, icon) in icons {
        let body = icon
            .get("body")
            .and_then(Value::as_str)
            .ok_or("Iconify icon body must be a string")?;
        retained_body_bytes = retained_body_bytes
            .checked_add(body.len())
            .ok_or("retained body accounting overflowed")?;
        let (elements, depth) = xml_statistics(body)?;
        max_xml_elements = max_xml_elements.max(elements);
        max_xml_depth = max_xml_depth.max(depth);
        if body.len() > max_body_bytes {
            max_body_bytes = body.len();
            largest_icon_name.clone_from(name);
        }
    }
    let (max_alias_depth, max_alias_fanout) = alias_statistics(icons, &aliases)?;
    let sha256 = format!("{:x}", Sha256::digest(bytes));

    Ok(PackStatistics {
        source_label: source_label.to_owned(),
        sha256,
        input_bytes: bytes.len(),
        prefix: prefix.clone(),
        last_modified: object.get("lastModified").and_then(Value::as_u64),
        json_members,
        max_json_depth,
        max_json_key_bytes,
        icons: icons.len(),
        aliases: aliases.len(),
        retained_body_bytes,
        max_body_bytes,
        max_xml_elements,
        max_xml_depth,
        max_alias_depth,
        max_alias_fanout,
        largest_icon_key: format!("{prefix}:{largest_icon_name}"),
    })
}

fn visit_json(
    value: &Value,
    depth: usize,
    members: &mut usize,
    max_depth: &mut usize,
    max_key_bytes: &mut usize,
) {
    match value {
        Value::Object(object) => {
            *max_depth = (*max_depth).max(depth);
            for (key, value) in object {
                *members = members.saturating_add(1);
                *max_key_bytes = (*max_key_bytes).max(key.len());
                visit_json(value, depth + 1, members, max_depth, max_key_bytes);
            }
        }
        Value::Array(values) => {
            *max_depth = (*max_depth).max(depth);
            for value in values {
                *members = members.saturating_add(1);
                visit_json(value, depth + 1, members, max_depth, max_key_bytes);
            }
        }
        _ => {}
    }
}

fn xml_statistics(body: &str) -> Result<(usize, usize), Box<dyn Error>> {
    let wrapped = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">{body}</svg>"#
    );
    let document = roxmltree::Document::parse(&wrapped)?;
    let wrapper = document.root_element();
    let mut elements = 0usize;
    let mut max_depth = 0usize;
    for node in document.descendants().filter(roxmltree::Node::is_element) {
        if node == wrapper {
            continue;
        }
        elements += 1;
        let mut depth = 0usize;
        let mut current = Some(node);
        while let Some(element) = current {
            if element == wrapper {
                break;
            }
            if element.is_element() {
                depth = depth.saturating_add(1);
            }
            current = element.parent();
        }
        max_depth = max_depth.max(depth);
    }
    Ok((elements, max_depth))
}

fn alias_statistics(
    icons: &Map<String, Value>,
    aliases: &Map<String, Value>,
) -> Result<(usize, usize), Box<dyn Error>> {
    let icon_names = icons.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let parents = aliases
        .iter()
        .map(|(name, alias)| {
            alias
                .get("parent")
                .and_then(Value::as_str)
                .map(|parent| (name.as_str(), parent))
                .ok_or_else(|| format!("alias {name:?} has no string parent"))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let mut fanout = BTreeMap::<&str, usize>::new();
    for parent in parents.values() {
        *fanout.entry(parent).or_default() += 1;
    }
    let mut max_depth = 0usize;
    for alias in parents.keys() {
        let mut current = *alias;
        let mut depth = 0usize;
        let mut seen = BTreeSet::new();
        while let Some(parent) = parents.get(current) {
            if !seen.insert(current) {
                return Err(format!("alias cycle at {current:?}").into());
            }
            depth += 1;
            current = parent;
        }
        if !icon_names.contains(current) {
            return Err(format!("missing alias parent {current:?}").into());
        }
        max_depth = max_depth.max(depth);
    }
    Ok((
        max_depth,
        fanout.values().copied().max().unwrap_or_default(),
    ))
}

fn aggregate_statistics(packs: &[PackStatistics]) -> AggregateStatistics {
    let input_bytes = packs.iter().map(|pack| pack.input_bytes).sum();
    let json_members = packs.iter().map(|pack| pack.json_members).sum();
    let icons = packs.iter().map(|pack| pack.icons).sum();
    let aliases = packs.iter().map(|pack| pack.aliases).sum();
    AggregateStatistics {
        packs: packs.len(),
        input_bytes,
        json_members,
        icons,
        aliases,
        total_entries: icons + aliases,
        retained_body_bytes: packs.iter().map(|pack| pack.retained_body_bytes).sum(),
        max_json_depth: packs
            .iter()
            .map(|pack| pack.max_json_depth)
            .max()
            .unwrap_or_default(),
        max_json_key_bytes: packs
            .iter()
            .map(|pack| pack.max_json_key_bytes)
            .max()
            .unwrap_or_default(),
        max_body_bytes: packs
            .iter()
            .map(|pack| pack.max_body_bytes)
            .max()
            .unwrap_or_default(),
        max_xml_elements: packs
            .iter()
            .map(|pack| pack.max_xml_elements)
            .max()
            .unwrap_or_default(),
        max_xml_depth: packs
            .iter()
            .map(|pack| pack.max_xml_depth)
            .max()
            .unwrap_or_default(),
        max_alias_depth: packs
            .iter()
            .map(|pack| pack.max_alias_depth)
            .max()
            .unwrap_or_default(),
        max_alias_fanout: packs
            .iter()
            .map(|pack| pack.max_alias_fanout)
            .max()
            .unwrap_or_default(),
    }
}

fn synthetic_body_alias_pack() -> Result<Vec<u8>, Box<dyn Error>> {
    let max_body_bytes = fixed_limit(IconRegistryResourceLimitId::MaxBodyBytes)?;
    let max_depth = fixed_limit(IconRegistryResourceLimitId::MaxXmlDepthPerBody)?;
    let max_alias_depth = fixed_limit(IconRegistryResourceLimitId::MaxAliasDepth)?;
    let max_alias_fanout = fixed_limit(IconRegistryResourceLimitId::MaxAliasFanout)?;

    let body_prefix = r#"<path data-padding=""#;
    let body_suffix = r#""/>"#;
    let padding = max_body_bytes
        .checked_sub(body_prefix.len() + body_suffix.len())
        .ok_or("max body limit is too small for the synthetic fixture")?;
    let max_body = format!("{body_prefix}{}{body_suffix}", "a".repeat(padding));
    let max_depth_body = format!("{}{}", "<g>".repeat(max_depth), "</g>".repeat(max_depth));

    let mut aliases = Map::new();
    for index in 0..max_alias_depth {
        let parent = if index == 0 {
            "root".to_owned()
        } else {
            format!("chain-{}", index - 1)
        };
        aliases.insert(format!("chain-{index}"), json!({ "parent": parent }));
    }
    for index in 0..max_alias_fanout.saturating_sub(1) {
        aliases.insert(format!("fanout-{index}"), json!({ "parent": "root" }));
    }

    Ok(serde_json::to_vec(&json!({
        "prefix": "calibration",
        "icons": {
            "root": { "body": "<path/>" },
            "max-body": { "body": max_body },
            "max-depth": { "body": max_depth_body },
        },
        "aliases": aliases,
    }))?)
}

fn synthetic_entry_edge_pack() -> Result<Vec<u8>, Box<dyn Error>> {
    let icon_count = fixed_limit(IconRegistryResourceLimitId::MaxIconEntries)?;
    let alias_count = fixed_limit(IconRegistryResourceLimitId::MaxAliasEntries)?;
    let total_entries = fixed_limit(IconRegistryResourceLimitId::MaxTotalEntries)?;
    let alias_edges = fixed_limit(IconRegistryResourceLimitId::MaxAliasEdges)?;
    if icon_count
        .checked_add(alias_count)
        .ok_or("synthetic entry count overflowed")?
        != total_entries
        || alias_count != alias_edges
    {
        return Err("entry/edge ceilings no longer support the registered synthetic graph".into());
    }

    let mut json = String::with_capacity(
        icon_count
            .checked_mul(40)
            .and_then(|bytes| bytes.checked_add(alias_count.saturating_mul(48)))
            .ok_or("synthetic entry pack capacity overflowed")?,
    );
    json.push_str(r#"{"prefix":"entries","icons":{"#);
    for index in 0..icon_count {
        if index > 0 {
            json.push(',');
        }
        write!(json, r#""i-{index}":{{"body":"<path/>"}}"#)?;
    }
    json.push_str(r#"},"aliases":{"#);
    for index in 0..alias_count {
        if index > 0 {
            json.push(',');
        }
        write!(json, r#""a-{index}":{{"parent":"i-{index}"}}"#)?;
    }
    json.push_str("}}");
    Ok(json.into_bytes())
}

fn synthetic_xml_rewrite_pack() -> Result<Vec<u8>, Box<dyn Error>> {
    const ID_ALPHABET: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_-";
    let element_count = fixed_limit(IconRegistryResourceLimitId::MaxXmlElementsPerBody)?;
    let edit_count = fixed_limit(IconRegistryResourceLimitId::MaxIdRewriteEditsPerBody)?;
    if element_count
        .checked_mul(4)
        .ok_or("synthetic XML edit count overflowed")?
        != edit_count
        || element_count > ID_ALPHABET.len().pow(2)
    {
        return Err("XML ceilings no longer support the registered four-edit fixture".into());
    }

    let mut body = String::with_capacity(
        element_count
            .checked_mul(58)
            .ok_or("synthetic XML body capacity overflowed")?,
    );
    for index in 0..element_count {
        let id = [
            ID_ALPHABET[index / ID_ALPHABET.len()],
            ID_ALPHABET[index % ID_ALPHABET.len()],
        ];
        let id = std::str::from_utf8(&id)?;
        write!(
            body,
            r##"<use id="{id}" href="#{id}" xlink:href="#{id}" fill="url(#{id})"/>"##
        )?;
    }
    if body.len() > fixed_limit(IconRegistryResourceLimitId::MaxBodyBytes)? {
        return Err("synthetic XML rewrite fixture exceeds the body ceiling".into());
    }

    Ok(serde_json::to_vec(&json!({
        "prefix": "rewrites",
        "icons": {
            "max-rewrites": { "body": body },
        },
    }))?)
}

fn fixed_limit(id: IconRegistryResourceLimitId) -> Result<usize, Box<dyn Error>> {
    Ok(usize::try_from(id.fixed_value())?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_json_depth_matches_constructor_root_depth() {
        let statistics = analyze_pack(
            "fixture.json",
            br#"{"prefix":"x","icons":{"a":{"body":"<path/>"}}}"#,
        )
        .unwrap();

        assert_eq!(statistics.max_json_depth, 3);
    }

    #[test]
    fn calibration_xml_statistics_skip_only_the_synthetic_wrapper() {
        let statistics = analyze_pack(
            "fixture.json",
            br#"{"prefix":"x","icons":{"a":{"body":"<svg><path/></svg>"}}}"#,
        )
        .unwrap();

        assert_eq!(statistics.max_xml_elements, 2);
        assert_eq!(statistics.max_xml_depth, 2);
    }
}
