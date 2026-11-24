#[path = "../native_memory/allocator.rs"]
mod allocator;
#[cfg(feature = "layout-elk")]
mod elk_hierarchy;
#[cfg(feature = "svg")]
mod flowchart;
#[cfg(all(feature = "svg", feature = "compat-json", feature = "layout-dagre"))]
mod flowchart_labels;
#[cfg(all(
    feature = "native-fonts",
    feature = "diagram-flowchart",
    feature = "layout-dagre"
))]
mod fonts;
#[cfg(all(
    feature = "all-diagrams",
    feature = "compat-json",
    feature = "editor-facts"
))]
mod parse;
#[cfg(all(feature = "svg", feature = "compat-json"))]
mod sequence;
#[cfg(all(
    feature = "benchmark-stages",
    feature = "diagram-flowchart",
    feature = "diagram-sequence",
    feature = "layout-elk"
))]
mod stages;
#[cfg(all(
    feature = "diagram-flowchart",
    feature = "diagram-sequence",
    feature = "layout-elk"
))]
mod workload;
#[cfg(all(
    feature = "diagram-flowchart",
    feature = "diagram-sequence",
    feature = "layout-elk"
))]
#[path = "../workloads/mod.rs"]
mod workloads;

use allocator::{AllocationMetrics, CountingSystemAllocator};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Read, Write};

const REQUEST_SCHEMA_VERSION: u32 = 1;
const MEMORY_SCALES: [u32; 6] = [1, 2, 4, 10, 32, 100];
const MAX_REQUEST_BYTES: u64 = 4096;
const USAGE: &str = "usage: memory [--mode workload [--stage-memory | --heap-profile <path>] | --mode scale-curve --lane <id> | --mode fonts]";

#[cfg(all(feature = "dhat-heap", not(target_arch = "wasm32")))]
type ProbeAllocator = CountingSystemAllocator<dhat::Alloc>;
#[cfg(not(all(feature = "dhat-heap", not(target_arch = "wasm32"))))]
type ProbeAllocator = CountingSystemAllocator;

#[cfg(all(feature = "dhat-heap", not(target_arch = "wasm32")))]
#[global_allocator]
static ALLOCATOR: ProbeAllocator = CountingSystemAllocator::new(dhat::Alloc);
#[cfg(not(all(feature = "dhat-heap", not(target_arch = "wasm32"))))]
#[global_allocator]
static ALLOCATOR: ProbeAllocator = CountingSystemAllocator::new(std::alloc::System);

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum Mode {
    Operation,
    Zero,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeRequest {
    schema_version: u32,
    lane_id: String,
    mode: Mode,
    scale: u32,
    seed: u64,
    repeat: u32,
    invocation_id: String,
    nonce: String,
}

impl ProbeRequest {
    fn read() -> Result<Self, ProbeError> {
        let mut bytes = Vec::new();
        io::stdin()
            .lock()
            .take(MAX_REQUEST_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| ProbeError::new(format!("failed to read request: {error}")))?;
        if bytes.len() as u64 > MAX_REQUEST_BYTES {
            return Err(ProbeError::new("request exceeds 4096 bytes"));
        }
        if bytes.last() != Some(&b'\n')
            || bytes.iter().filter(|byte| **byte == b'\n').count() != 1
            || bytes.contains(&b'\r')
        {
            return Err(ProbeError::new(
                "request must contain exactly one newline-terminated JSON line",
            ));
        }
        serde_json::from_slice(&bytes[..bytes.len() - 1])
            .map_err(|error| ProbeError::new(format!("invalid request JSON: {error}")))
    }

    fn validate(&self) -> Result<(), ProbeError> {
        if self.schema_version != REQUEST_SCHEMA_VERSION {
            return Err(ProbeError::new("unsupported schema_version"));
        }
        if !MEMORY_SCALES.contains(&self.scale) {
            return Err(ProbeError::new("unsupported memory scale"));
        }
        if self.invocation_id.is_empty()
            || self.invocation_id.trim() != self.invocation_id
            || self.invocation_id.len() > 256
        {
            return Err(ProbeError::new(
                "invocation_id must be trimmed, non-empty, and at most 256 bytes",
            ));
        }
        if self.nonce.len() != 32
            || !self
                .nonce
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(ProbeError::new(
                "nonce must be 32-character lowercase hexadecimal",
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ProbeError(String);

impl ProbeError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

#[cfg(any(feature = "compat-json", feature = "layout-elk"))]
#[derive(Serialize)]
struct SemanticReport<T> {
    workload_units: u32,
    semantic_output: T,
}

#[derive(Serialize)]
struct ProbeResponse<'a, T> {
    schema_version: u32,
    lane_id: &'a str,
    public_operation: &'static str,
    process_lifecycle: &'static str,
    engine_lifecycle: &'static str,
    logical_operations_per_estimate: u32,
    mode: Mode,
    scale: u32,
    seed: u64,
    repeat: u32,
    pid: u32,
    executable_sha256: &'a str,
    invocation_id: &'a str,
    nonce: &'a str,
    output_sha256: String,
    #[serde(flatten)]
    output: T,
    snapshot_live_bytes: u64,
    allocation_count: u64,
    allocated_bytes: u64,
    live_bytes_after: u64,
    peak_live_bytes: u64,
    peak_growth_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    live_bytes_after_drop: Option<u64>,
    counter_overflowed: bool,
    counter_underflowed: bool,
}

struct Probe {
    request: ProbeRequest,
    executable_sha256: String,
    schema_version: u32,
}

impl Probe {
    fn report(
        &self,
        public_operation: &'static str,
        output_sha256: String,
        output: impl Serialize,
        metrics: AllocationMetrics,
        live_bytes_after_drop: Option<u64>,
    ) {
        let request = &self.request;
        write_response(&ProbeResponse {
            schema_version: self.schema_version,
            lane_id: &request.lane_id,
            public_operation,
            process_lifecycle: "fresh-process",
            engine_lifecycle: "reused-engine",
            logical_operations_per_estimate: 1,
            mode: request.mode,
            scale: request.scale,
            seed: request.seed,
            repeat: request.repeat,
            pid: std::process::id(),
            executable_sha256: &self.executable_sha256,
            invocation_id: &request.invocation_id,
            nonce: &request.nonce,
            output_sha256,
            output,
            snapshot_live_bytes: metrics.snapshot_live_bytes,
            allocation_count: metrics.allocation_count,
            allocated_bytes: metrics.allocated_bytes,
            live_bytes_after: metrics.live_bytes_after,
            peak_live_bytes: metrics.peak_live_bytes,
            peak_growth_bytes: metrics.peak_growth_bytes,
            live_bytes_after_drop,
            counter_overflowed: metrics.counter_overflowed,
            counter_underflowed: metrics.counter_underflowed,
        });
    }
}

fn sha256_bytes(bytes: &[u8]) -> String {
    lower_hex(&Sha256::digest(bytes))
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

fn executable_sha256() -> Result<String, ProbeError> {
    let path = std::env::current_exe()
        .map_err(|error| ProbeError::new(format!("failed to locate executable: {error}")))?;
    let mut file = File::open(path)
        .map_err(|error| ProbeError::new(format!("failed to open executable: {error}")))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| ProbeError::new(format!("failed to hash executable: {error}")))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(lower_hex(&digest.finalize()))
}

fn write_json_line(value: &impl Serialize) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer(&mut stdout, value)?;
    stdout.write_all(b"\n")?;
    stdout.flush()
}

fn write_response(value: &impl Serialize) {
    if write_json_line(value).is_err() {
        std::process::exit(2);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lane {
    #[cfg(feature = "svg")]
    Flowchart,
    #[cfg(all(feature = "svg", feature = "compat-json", feature = "layout-dagre"))]
    FlowchartLabels,
    #[cfg(all(feature = "svg", feature = "compat-json"))]
    Sequence(sequence::MessageProfile),
    #[cfg(feature = "layout-elk")]
    ElkHierarchy,
    #[cfg(all(
        feature = "all-diagrams",
        feature = "compat-json",
        feature = "editor-facts"
    ))]
    Parse(parse::Family, parse::Route),
}

impl Lane {
    fn from_id(id: &str) -> Result<Self, ProbeError> {
        match id {
            #[cfg(feature = "svg")]
            flowchart::LANE_ID => Ok(Self::Flowchart),
            #[cfg(all(feature = "svg", feature = "compat-json", feature = "layout-dagre"))]
            flowchart_labels::LANE_ID => Ok(Self::FlowchartLabels),
            #[cfg(feature = "layout-elk")]
            elk_hierarchy::LANE_ID => Ok(Self::ElkHierarchy),
            _ => {
                #[cfg(all(feature = "svg", feature = "compat-json"))]
                if let Some(profile) = sequence::MessageProfile::from_lane_id(id) {
                    return Ok(Self::Sequence(profile));
                }
                #[cfg(all(
                    feature = "all-diagrams",
                    feature = "compat-json",
                    feature = "editor-facts"
                ))]
                if let Some((family, route)) = parse::selected_lane(id) {
                    return Ok(Self::Parse(family, route));
                }
                Err(ProbeError::new("unsupported lane_id for this memory build"))
            }
        }
    }

    const fn schema_version(self) -> u32 {
        match self {
            #[cfg(feature = "svg")]
            Self::Flowchart => 1,
            #[cfg(feature = "layout-elk")]
            Self::ElkHierarchy => 2,
            #[cfg(all(feature = "svg", feature = "compat-json", feature = "layout-dagre"))]
            Self::FlowchartLabels => 3,
            #[cfg(all(feature = "svg", feature = "compat-json"))]
            Self::Sequence(_) => 3,
            #[cfg(all(
                feature = "all-diagrams",
                feature = "compat-json",
                feature = "editor-facts"
            ))]
            Self::Parse(..) => 3,
        }
    }

    fn measure(self, probe: &Probe) -> Result<(), ProbeError> {
        match self {
            #[cfg(feature = "svg")]
            Self::Flowchart => flowchart::measure(probe),
            #[cfg(all(feature = "svg", feature = "compat-json", feature = "layout-dagre"))]
            Self::FlowchartLabels => flowchart_labels::measure(probe),
            #[cfg(all(feature = "svg", feature = "compat-json"))]
            Self::Sequence(profile) => sequence::measure(probe, profile),
            #[cfg(feature = "layout-elk")]
            Self::ElkHierarchy => elk_hierarchy::measure(probe),
            #[cfg(all(
                feature = "all-diagrams",
                feature = "compat-json",
                feature = "editor-facts"
            ))]
            Self::Parse(family, route) => {
                parse::measure(probe, family, route).map_err(ProbeError::new)
            }
        }
    }
}

fn scale_curve(expected_lane: Option<Lane>) {
    std::panic::set_hook(Box::new(|_| {}));
    let mut schema_version = expected_lane.map_or(1, Lane::schema_version);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let request = ProbeRequest::read()?;
        let lane = Lane::from_id(&request.lane_id)?;
        if expected_lane.is_some_and(|expected| expected != lane) {
            return Err(ProbeError::new("request lane_id differs from --lane"));
        }
        schema_version = lane.schema_version();
        request.validate()?;
        let probe = Probe {
            request,
            executable_sha256: executable_sha256()?,
            schema_version,
        };
        lane.measure(&probe)
    }));
    let error = match result {
        Ok(Ok(())) => return,
        Ok(Err(ProbeError(error))) => error,
        Err(_) => "native memory probe panicked".to_owned(),
    };
    ALLOCATOR.stop_measurement();
    let _ = write_json_line(&serde_json::json!({"schema_version": schema_version, "error": error}));
    std::process::exit(2);
}

enum Command {
    ScaleCurve(Option<Lane>),
    #[cfg(all(
        feature = "diagram-flowchart",
        feature = "diagram-sequence",
        feature = "layout-elk"
    ))]
    Workload(WorkloadMode),
    #[cfg(all(
        feature = "native-fonts",
        feature = "diagram-flowchart",
        feature = "layout-dagre"
    ))]
    Fonts,
    #[cfg(all(
        feature = "all-diagrams",
        feature = "compat-json",
        feature = "editor-facts"
    ))]
    StateShape,
    Help,
}

#[cfg(all(
    feature = "diagram-flowchart",
    feature = "diagram-sequence",
    feature = "layout-elk"
))]
enum WorkloadMode {
    Counters,
    Stages,
    #[cfg(all(feature = "dhat-heap", not(target_arch = "wasm32")))]
    HeapProfile(std::path::PathBuf),
}

#[cfg(all(
    feature = "diagram-flowchart",
    feature = "diagram-sequence",
    feature = "layout-elk"
))]
fn workload_command(mut args: impl Iterator<Item = OsString>) -> Result<Command, String> {
    match args.next() {
        None => Ok(Command::Workload(WorkloadMode::Counters)),
        Some(flag) if flag == "--stage-memory" && args.next().is_none() => {
            Ok(Command::Workload(WorkloadMode::Stages))
        }
        Some(flag) if flag == "--heap-profile" => {
            #[cfg(all(feature = "dhat-heap", not(target_arch = "wasm32")))]
            {
                let path = args.next().ok_or(USAGE)?;
                if args.next().is_some() {
                    return Err(USAGE.to_owned());
                }
                Ok(Command::Workload(WorkloadMode::HeapProfile(path.into())))
            }
            #[cfg(not(all(feature = "dhat-heap", not(target_arch = "wasm32"))))]
            Err("heap profiling requires the dhat-heap feature on a native target".into())
        }
        _ => Err(USAGE.to_owned()),
    }
}

#[cfg(not(all(
    feature = "diagram-flowchart",
    feature = "diagram-sequence",
    feature = "layout-elk"
)))]
fn workload_command(_: impl Iterator<Item = OsString>) -> Result<Command, String> {
    Err("workload mode requires diagram-flowchart, diagram-sequence, and layout-elk".into())
}

fn command() -> Result<Command, String> {
    let mut args = std::env::args_os().skip(1);
    let Some(flag) = args.next() else {
        return Ok(Command::ScaleCurve(None));
    };
    match flag.to_str() {
        Some("--workload-stdin") => workload_command(args),
        Some("--help" | "-h") if args.next().is_none() => Ok(Command::Help),
        #[cfg(all(
            feature = "all-diagrams",
            feature = "compat-json",
            feature = "editor-facts"
        ))]
        Some("--state-shape") => Ok(Command::StateShape),
        Some("--mode") => match args.next().as_deref().and_then(OsStr::to_str) {
            Some("workload") => workload_command(args),
            #[cfg(all(
                feature = "native-fonts",
                feature = "diagram-flowchart",
                feature = "layout-dagre"
            ))]
            Some("fonts") if args.next().is_none() => Ok(Command::Fonts),
            Some("scale-curve") if args.next().as_deref() == Some(OsStr::new("--lane")) => {
                let id = args.next().ok_or(USAGE)?;
                if args.next().is_some() {
                    return Err(USAGE.to_owned());
                }
                let lane = Lane::from_id(id.to_str().ok_or(USAGE)?).map_err(|error| error.0)?;
                Ok(Command::ScaleCurve(Some(lane)))
            }
            _ => Err(USAGE.to_owned()),
        },
        _ => Err(USAGE.to_owned()),
    }
}

pub(super) fn run() -> Result<(), String> {
    match command()? {
        Command::ScaleCurve(lane) => scale_curve(lane),
        #[cfg(all(
            feature = "diagram-flowchart",
            feature = "diagram-sequence",
            feature = "layout-elk"
        ))]
        Command::Workload(mode) => return workload::measure(mode),
        #[cfg(all(
            feature = "native-fonts",
            feature = "diagram-flowchart",
            feature = "layout-dagre"
        ))]
        Command::Fonts => fonts::observe(),
        #[cfg(all(
            feature = "all-diagrams",
            feature = "compat-json",
            feature = "editor-facts"
        ))]
        Command::StateShape => {
            std::panic::set_hook(Box::new(|_| {}));
            parse::run_state_shape_mode();
        }
        Command::Help => println!("{USAGE}"),
    }
    Ok(())
}
