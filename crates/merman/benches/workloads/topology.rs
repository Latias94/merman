use merman::svg::{RenderCapability, RenderResourcePolicy, ResourceLimitId};
use merman::{
    OperationControl, RenderError, RenderOutput, RenderRequest, Renderer, ResourceLimitCause,
    SvgRequest,
};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const WORK: ResourceLimitId = ResourceLimitId::MaxLayoutWorkUnits;
const HIGH_BUDGET: usize = 2_000_000_000;

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
enum Outcome<T> {
    Accepted(T),
    LayoutWorkRejected {
        id: &'static str,
        phase: &'static str,
        actual: u64,
        maximum: u64,
    },
    SourceMaxEdgesRejected {
        code: String,
        message: String,
    },
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct Geometry {
    sha256: String,
    bytes: usize,
    nodes: usize,
    edges: usize,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
struct Svg {
    sha256: String,
    bytes: usize,
    layout_work_units: usize,
}

fn svg_request(budget: Option<usize>) -> Result<SvgRequest, String> {
    let mut request = SvgRequest::default();
    if let Some(budget) = budget {
        let policy = RenderResourcePolicy::default()
            .with_limit(WORK, budget)
            .map_err(|e| e.to_string())?;
        request.environment = request.environment.with_resource_policy(policy);
    }
    Ok(request)
}

fn layout_signature(bytes: &[u8], nodes: usize, edges: usize) -> Result<Outcome<Geometry>, String> {
    let geometry: Value =
        serde_json::from_slice(bytes).map_err(|e| format!("invalid layout JSON: {e}"))?;
    let flowchart = &geometry["layout"]["FlowchartV2"];
    for (field, expected) in [("nodes", nodes), ("edges", edges)] {
        let items = flowchart[field]
            .as_array()
            .ok_or_else(|| format!("missing FlowchartV2 {field} array"))?;
        if items.len() != expected {
            return Err(format!("expected {expected} {field}, got {}", items.len()));
        }
    }
    Ok(Outcome::Accepted(Geometry {
        sha256: data_encoding::HEXLOWER.encode(&Sha256::digest(bytes)),
        bytes: bytes.len(),
        nodes,
        edges,
    }))
}

fn classify_error<T>(error: RenderError, budget: usize) -> Result<Outcome<T>, String> {
    match error {
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == WORK.as_str()
                && limit.phase == "layout_model"
                && limit.cause == ResourceLimitCause::Ceiling
                && limit.maximum == budget as u64
                && limit.actual > limit.maximum =>
        {
            Ok(Outcome::LayoutWorkRejected {
                id: limit.id,
                phase: limit.phase,
                actual: limit.actual,
                maximum: limit.maximum,
            })
        }
        RenderError::Parse(diagnostic) => {
            let details = diagnostic.terminal_diagnostic_details();
            let message = diagnostic.terminal_safe_message();
            // The Flowchart parser currently exposes maxEdges through its ordinary parse diagnostic.
            if details.diagram_type.as_deref() == Some("flowchart-v2")
                && message.starts_with(
                    "Diagram parse error (flowchart-v2): Flowchart edge limit exceeded:",
                )
                && message.contains("maxEdges budget")
            {
                Ok(Outcome::SourceMaxEdgesRejected {
                    code: details.code,
                    message,
                })
            } else {
                Err(message)
            }
        }
        other => Err(other.to_string()),
    }
}

fn svg_outcome(
    renderer: &Renderer,
    source: &str,
    budget: Option<usize>,
    default_budget: usize,
) -> Result<Outcome<Svg>, String> {
    let request = RenderRequest::svg(source, OperationControl::new(), svg_request(budget)?);
    let svg = match renderer.render(request) {
        Ok(RenderOutput::Svg(Some(svg))) => svg,
        Ok(_) => return Err("missing SVG work evidence".into()),
        Err(error) => return classify_error(error, budget.unwrap_or(default_budget)),
    };
    if !svg
        .evidence()
        .required_capabilities()
        .contains(&RenderCapability::LayoutElk)
    {
        return Err("topology workload requires the default ELK backend".into());
    }
    Ok(Outcome::Accepted(Svg {
        sha256: data_encoding::HEXLOWER.encode(&Sha256::digest(svg.svg().as_bytes())),
        bytes: svg.svg().len(),
        layout_work_units: svg.evidence().layout_work_units(),
    }))
}

pub(super) fn observe(
    renderer: &Renderer,
    source: &str,
    nodes: usize,
    edges: usize,
) -> Result<Value, String> {
    let default_budget = RenderResourcePolicy::default()
        .value(WORK)
        .ok_or("missing default layout-work ceiling")?;
    let layout = |budget: Option<usize>| {
        let request =
            RenderRequest::layout_json(source, OperationControl::new(), svg_request(budget)?);
        let mut bytes = Vec::new();
        match renderer.write_layout_json(request, &mut bytes, false) {
            Ok(true) => layout_signature(&bytes, nodes, edges),
            Ok(false) => Err("topology source produced no diagram".into()),
            Err(e) => classify_error(e, budget.unwrap_or(default_budget)),
        }
    };

    let default = layout(None)?;
    let high = layout(Some(HIGH_BUDGET))?;
    let default_svg = svg_outcome(renderer, source, None, default_budget)?;
    let high_svg = svg_outcome(renderer, source, Some(HIGH_BUDGET), default_budget)?;
    let boundary = match (&high, &high_svg) {
        (Outcome::SourceMaxEdgesRejected { .. }, Outcome::SourceMaxEdgesRejected { .. })
            if default == high && default_svg == high_svg =>
        {
            Value::Null
        }
        (Outcome::Accepted(_), Outcome::Accepted(svg)) => {
            match &default_svg {
                Outcome::Accepted(output) if output.sha256 == svg.sha256 => {}
                Outcome::LayoutWorkRejected { .. } => {}
                other => {
                    return Err(format!(
                        "default and high-budget SVG disagree: {other:?}, {svg:?}"
                    ));
                }
            }
            let upper = match &default {
                Outcome::LayoutWorkRejected { .. } => HIGH_BUDGET,
                _ if default == high => default_budget,
                _ => {
                    return Err(format!(
                        "default and high-budget layout disagree: {default:?}, {high:?}"
                    ));
                }
            };
            let consumed = svg.layout_work_units;
            let (minimum, below) =
                minimum_budget(|budget| layout(Some(budget)), &high, upper, consumed)?;
            json!({
                "backend": "elk",
                "svg_consumed_layout_work_units": consumed,
                "minimum_passing_budget": minimum,
                "minimum_minus_one": below,
            })
        }
        _ => {
            return Err(format!(
                "unexpected topology outcomes: layout {default:?}/{high:?}, SVG {default_svg:?}/{high_svg:?}"
            ));
        }
    };
    Ok(json!({
        "nodes": nodes,
        "edges": edges,
        "default_budget": default_budget,
        "default": default,
        "high_budget": HIGH_BUDGET,
        "high": high,
        "svg": {"default": default_svg, "high": high_svg},
        "boundary": boundary,
    }))
}

fn minimum_budget(
    probe: impl Fn(usize) -> Result<Outcome<Geometry>, String>,
    reference: &Outcome<Geometry>,
    mut passing: usize,
    consumed: usize,
) -> Result<(usize, Outcome<Geometry>), String> {
    // On this fixed graph only the ceiling varies. ELK can check work without consuming it,
    // so SVG evidence is a hint. Both adjacent limits must be observed on the layout writer.
    let mut hints = [consumed, consumed.saturating_sub(1)].into_iter();
    let mut failing = 0;
    let mut rejection = None;
    while passing - failing > 1 {
        let budget = hints
            .find(|&n| n > failing && n < passing)
            .unwrap_or_else(|| failing + (passing - failing) / 2);
        match probe(budget)? {
            outcome if &outcome == reference => passing = budget,
            outcome @ Outcome::LayoutWorkRejected { actual, .. } if actual <= passing as u64 => {
                failing = budget;
                rejection = Some(outcome);
            }
            other => return Err(format!("inconsistent layout at budget {budget}: {other:?}")),
        }
    }
    match rejection {
        Some(
            below @ Outcome::LayoutWorkRejected {
                actual, maximum, ..
            },
        ) if actual == passing as u64 && maximum == (passing - 1) as u64 => Ok((passing, below)),
        other => Err(format!(
            "inconsistent rejection immediately below {passing}: {other:?}"
        )),
    }
}
