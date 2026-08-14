#[path = "native_memory/allocator.rs"]
mod allocator;

use allocator::CountingSystemAllocator;
use merman::svg::{RenderResourcePolicy, SvgRenderOptions};
use merman::{
    Engine, OperationControl, ParseOptions, RenderOutput, RenderRequest, Renderer, SvgEnvironment,
    SvgRequest, runtime::RuntimePolicy,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;

const SCHEMA_VERSION: u32 = 1;
const LANE_ID: &str = "flowchart-end-to-end-memory";
const PUBLIC_OPERATION: &str = "render-svg";
const PROCESS_LIFECYCLE: &str = "fresh-process";
const ENGINE_LIFECYCLE: &str = "reused-engine";
const LOGICAL_OPERATIONS_PER_ESTIMATE: u32 = 1;
const MEMORY_SCALES: [u32; 6] = [1, 2, 4, 10, 32, 100];
const MAX_REQUEST_BYTES: u64 = 4 * 1024;
const NODES_PER_SCALE: u64 = 3;
const EDGES_PER_SCALE: u64 = 4;

#[global_allocator]
static ALLOCATOR: CountingSystemAllocator = CountingSystemAllocator::new();

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

#[derive(Debug, Serialize)]
struct ProbeResponse {
    schema_version: u32,
    lane_id: String,
    public_operation: &'static str,
    process_lifecycle: &'static str,
    engine_lifecycle: &'static str,
    logical_operations_per_estimate: u32,
    mode: Mode,
    scale: u32,
    seed: u64,
    repeat: u32,
    pid: u32,
    executable_sha256: String,
    invocation_id: String,
    nonce: String,
    output_sha256: String,
    output_width: f64,
    output_height: f64,
    input_nodes: u64,
    input_edges: u64,
    snapshot_live_bytes: u64,
    allocation_count: u64,
    allocated_bytes: u64,
    live_bytes_after: u64,
    peak_live_bytes: u64,
    peak_growth_bytes: u64,
    counter_overflowed: bool,
    counter_underflowed: bool,
}

#[derive(Debug, Serialize)]
struct ErrorResponse<'a> {
    schema_version: u32,
    error: &'a str,
}

#[derive(Debug)]
struct ProbeError(String);

impl ProbeError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

fn read_request() -> Result<ProbeRequest, ProbeError> {
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

fn validate_request(request: &ProbeRequest) -> Result<(), ProbeError> {
    if request.schema_version != SCHEMA_VERSION {
        return Err(ProbeError::new("unsupported schema_version"));
    }
    if request.lane_id != LANE_ID {
        return Err(ProbeError::new("unsupported lane_id"));
    }
    if !MEMORY_SCALES.contains(&request.scale) {
        return Err(ProbeError::new("unsupported memory scale"));
    }
    if request.invocation_id.is_empty()
        || request.invocation_id.trim() != request.invocation_id
        || request.invocation_id.len() > 256
    {
        return Err(ProbeError::new(
            "invocation_id must be trimmed, non-empty, and at most 256 bytes",
        ));
    }
    if request.nonce.len() != 32
        || !request
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

fn build_flowchart(scale: u32, seed: u64) -> Result<String, ProbeError> {
    let mut source = String::with_capacity(scale as usize * 640);
    source.push_str("flowchart LR\n");
    let last_node = NODES_PER_SCALE - 1;

    for group in 0..scale {
        writeln!(&mut source, "  subgraph sg{group}[\"Partition {group}\"]")
            .map_err(|_| ProbeError::new("failed to construct flowchart"))?;
        for node in 0..NODES_PER_SCALE {
            writeln!(
                &mut source,
                "    n{group}_{node}[\"Node {group}:{node}:{seed:016x}\"]"
            )
            .map_err(|_| ProbeError::new("failed to construct flowchart"))?;
        }
        for node in 0..(NODES_PER_SCALE - 1) {
            writeln!(&mut source, "    n{group}_{node} --> n{group}_{}", node + 1)
                .map_err(|_| ProbeError::new("failed to construct flowchart"))?;
        }
        source.push_str("  end\n");
    }

    for group in 0..scale {
        let next = (group + 1) % scale;
        let jump = (group as u64 + 1 + seed % u64::from(scale)) % u64::from(scale);
        writeln!(&mut source, "  n{group}_{last_node} --> n{next}_0")
            .map_err(|_| ProbeError::new("failed to construct flowchart"))?;
        writeln!(&mut source, "  n{group}_0 --> n{jump}_{last_node}")
            .map_err(|_| ProbeError::new("failed to construct flowchart"))?;
    }

    Ok(source)
}

fn sha256_file(path: &Path) -> Result<String, ProbeError> {
    let mut file = File::open(path)
        .map_err(|error| ProbeError::new(format!("failed to open executable: {error}")))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|error| ProbeError::new(format!("failed to hash executable: {error}")))?;
        if bytes_read == 0 {
            break;
        }
        digest.update(&buffer[..bytes_read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn executable_sha256() -> Result<String, ProbeError> {
    let path = std::env::current_exe()
        .map_err(|error| ProbeError::new(format!("failed to locate executable: {error}")))?;
    sha256_file(&path)
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn svg_dimensions(svg: &str) -> Result<(f64, f64), ProbeError> {
    let document = roxmltree::Document::parse(svg)
        .map_err(|error| ProbeError::new(format!("invalid rendered SVG: {error}")))?;
    let root = document.root_element();
    if root.tag_name().name() != "svg" {
        return Err(ProbeError::new("rendered output root is not svg"));
    }
    let view_box = root
        .attribute("viewBox")
        .ok_or_else(|| ProbeError::new("rendered SVG has no viewBox"))?;
    let values = view_box
        .split(|character: char| character.is_ascii_whitespace() || character == ',')
        .filter(|value| !value.is_empty())
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| ProbeError::new(format!("invalid SVG viewBox: {error}")))?;
    if values.len() != 4 {
        return Err(ProbeError::new("SVG viewBox must contain four numbers"));
    }
    let width = values[2];
    let height = values[3];
    if !width.is_finite() || width < 0.0 || !height.is_finite() || height < 0.0 {
        return Err(ProbeError::new(
            "SVG viewBox dimensions must be finite and non-negative",
        ));
    }
    Ok((width, height))
}

fn execute_probe() -> Result<ProbeResponse, ProbeError> {
    if std::env::args_os().nth(1).is_some() {
        return Err(ProbeError::new("native memory probe accepts no arguments"));
    }

    let request = read_request()?;
    validate_request(&request)?;
    let executable_sha256 = executable_sha256()?;
    let source = build_flowchart(request.scale, request.seed)?;
    // This fresh-process probe renders repository-generated trusted input under an outer timeout.
    // Disable configurable policy budgets so the full scale curve measures allocator behavior
    // without coupling its largest fixture to a product resource-profile ceiling; hard backend
    // capabilities remain enforced by this profile.
    let render_policy = RenderResourcePolicy::unbounded_for_trusted_input();
    let renderer = Renderer::new()
        .with_engine(
            Engine::new()
                .with_runtime_policy(RuntimePolicy::deterministic().with_fixed_seed(request.seed)),
        )
        .with_parse_options(ParseOptions::strict())
        .with_resource_policy(*render_policy.input_policy());
    let input_nodes = NODES_PER_SCALE * u64::from(request.scale);
    let input_edges = EDGES_PER_SCALE * u64::from(request.scale);

    let snapshot_live_bytes = ALLOCATOR.begin_measurement();
    let render_result = match request.mode {
        Mode::Operation => {
            let output = renderer
                .render(
                    RenderRequest::svg(
                        &source,
                        OperationControl::new(),
                        SvgRequest {
                            environment: SvgEnvironment::deterministic()
                                .with_resource_policy(render_policy),
                            options: SvgRenderOptions {
                                diagram_id: Some("native-memory-flowchart".to_string()),
                                ..Default::default()
                            },
                            ..Default::default()
                        },
                    )
                    .with_parse_options(ParseOptions::strict())
                    .with_resource_policy(*render_policy.input_policy()),
                )
                .map_err(|error| ProbeError::new(format!("flowchart render failed: {error}")))?;
            let RenderOutput::Svg(Some(svg)) = output else {
                return Err(ProbeError::new("flowchart render returned no diagram"));
            };
            Some(svg.into_parts().0)
        }
        Mode::Zero => None,
    };
    let metrics = ALLOCATOR.finish_measurement(snapshot_live_bytes);

    let svg = render_result;
    let (output_sha256, output_width, output_height) = match svg.as_deref() {
        Some(svg) => {
            let dimensions = svg_dimensions(svg)?;
            (sha256_bytes(svg.as_bytes()), dimensions.0, dimensions.1)
        }
        None => (sha256_bytes(&[]), 0.0, 0.0),
    };

    Ok(ProbeResponse {
        schema_version: SCHEMA_VERSION,
        lane_id: request.lane_id,
        public_operation: PUBLIC_OPERATION,
        process_lifecycle: PROCESS_LIFECYCLE,
        engine_lifecycle: ENGINE_LIFECYCLE,
        logical_operations_per_estimate: LOGICAL_OPERATIONS_PER_ESTIMATE,
        mode: request.mode,
        scale: request.scale,
        seed: request.seed,
        repeat: request.repeat,
        pid: std::process::id(),
        executable_sha256,
        invocation_id: request.invocation_id,
        nonce: request.nonce,
        output_sha256,
        output_width,
        output_height,
        input_nodes,
        input_edges,
        snapshot_live_bytes: metrics.snapshot_live_bytes,
        allocation_count: metrics.allocation_count,
        allocated_bytes: metrics.allocated_bytes,
        live_bytes_after: metrics.live_bytes_after,
        peak_live_bytes: metrics.peak_live_bytes,
        peak_growth_bytes: metrics.peak_growth_bytes,
        counter_overflowed: metrics.counter_overflowed,
        counter_underflowed: metrics.counter_underflowed,
    })
}

fn write_json_line<T: Serialize>(value: &T) -> io::Result<()> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    serde_json::to_writer(&mut lock, value)?;
    lock.write_all(b"\n")?;
    lock.flush()
}

fn fail(message: &str) -> ! {
    ALLOCATOR.stop_measurement();
    let response = ErrorResponse {
        schema_version: SCHEMA_VERSION,
        error: message,
    };
    let _ = write_json_line(&response);
    std::process::exit(2);
}

fn main() {
    std::panic::set_hook(Box::new(|_| {}));
    match std::panic::catch_unwind(execute_probe) {
        Ok(Ok(response)) => {
            if write_json_line(&response).is_err() {
                std::process::exit(2);
            }
        }
        Ok(Err(error)) => fail(&error.0),
        Err(_) => fail("native memory probe panicked"),
    }
}
