use super::{ALLOCATOR, Mode, Probe, ProbeError, sha256_bytes};
use merman::svg::{RenderResourcePolicy, SvgRenderOptions};
use merman::{
    MermaidEngine, OperationControl, ParseOptions, RenderOutput, RenderRequest, Renderer,
    SvgEnvironment, SvgRequest, model::runtime::RuntimePolicy,
};
use serde::Serialize;
use std::fmt::Write as _;

pub(super) const LANE_ID: &str = "flowchart-end-to-end-memory";
const PUBLIC_OPERATION: &str = "render-svg";
const NODES_PER_SCALE: u64 = 3;
const EDGES_PER_SCALE: u64 = 4;

#[derive(Serialize)]
struct SvgReport {
    output_width: f64,
    output_height: f64,
    input_nodes: u64,
    input_edges: u64,
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

pub(super) fn measure(probe: &Probe) -> Result<(), ProbeError> {
    let request = &probe.request;
    let source = build_flowchart(request.scale, request.seed)?;
    // This fresh-process probe renders repository-generated trusted input under an outer timeout.
    // Disable configurable policy budgets so the full scale curve measures allocator behavior
    // without coupling its largest fixture to a product resource-profile ceiling; hard backend
    // capabilities remain enforced by this profile.
    let render_policy = RenderResourcePolicy::unbounded_for_trusted_input();
    let renderer = Renderer::new()
        .with_presentation_engine(
            MermaidEngine::new()
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

    probe.report(
        PUBLIC_OPERATION,
        output_sha256,
        SvgReport {
            output_width,
            output_height,
            input_nodes,
            input_edges,
        },
        metrics,
        None,
    );
    Ok(())
}
