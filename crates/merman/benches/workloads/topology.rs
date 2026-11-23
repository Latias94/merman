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
enum LayoutOutcome {
    Accepted {
        sha256: String,
        bytes: usize,
        nodes: usize,
        edges: usize,
    },
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

fn layout_signature(bytes: &[u8], nodes: usize, edges: usize) -> Result<LayoutOutcome, String> {
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
    Ok(LayoutOutcome::Accepted {
        sha256: data_encoding::HEXLOWER.encode(&Sha256::digest(bytes)),
        bytes: bytes.len(),
        nodes,
        edges,
    })
}

fn classify_error(error: RenderError, budget: usize) -> Result<LayoutOutcome, String> {
    match error {
        RenderError::ResourceLimitExceeded(limit)
            if limit.id == WORK.as_str()
                && limit.phase == "layout_model"
                && limit.cause == ResourceLimitCause::Ceiling
                && limit.maximum == budget as u64
                && limit.actual > limit.maximum =>
        {
            Ok(LayoutOutcome::LayoutWorkRejected {
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
                Ok(LayoutOutcome::SourceMaxEdgesRejected {
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

fn svg_consumed_work(renderer: &Renderer, source: &str) -> Result<usize, String> {
    let request = RenderRequest::svg(
        source,
        OperationControl::new(),
        svg_request(Some(HIGH_BUDGET))?,
    );
    let RenderOutput::Svg(Some(svg)) = renderer.render(request).map_err(|e| e.to_string())? else {
        return Err("missing SVG work evidence".into());
    };
    if !svg
        .evidence()
        .required_capabilities()
        .contains(&RenderCapability::LayoutElk)
    {
        return Err("topology workload requires the default ELK backend".into());
    }
    Ok(svg.evidence().layout_work_units())
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
    let boundary = match (&default, &high) {
        (LayoutOutcome::SourceMaxEdgesRejected { .. }, _) if default == high => Value::Null,
        (_, LayoutOutcome::Accepted { .. }) => {
            let upper = match &default {
                LayoutOutcome::LayoutWorkRejected { .. } => HIGH_BUDGET,
                _ if default == high => default_budget,
                _ => {
                    return Err(format!(
                        "default and high-budget layout disagree: {default:?}, {high:?}"
                    ));
                }
            };
            let consumed = svg_consumed_work(renderer, source)?;
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
                "unexpected high-budget layout: {high:?} (default: {default:?})"
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
        "boundary": boundary,
    }))
}

fn minimum_budget(
    probe: impl Fn(usize) -> Result<LayoutOutcome, String>,
    reference: &LayoutOutcome,
    mut passing: usize,
    consumed: usize,
) -> Result<(usize, LayoutOutcome), String> {
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
            outcome @ LayoutOutcome::LayoutWorkRejected { actual, .. }
                if actual <= passing as u64 =>
            {
                failing = budget;
                rejection = Some(outcome);
            }
            other => return Err(format!("inconsistent layout at budget {budget}: {other:?}")),
        }
    }
    match rejection {
        Some(
            below @ LayoutOutcome::LayoutWorkRejected {
                actual, maximum, ..
            },
        ) if actual == passing as u64 && maximum == (passing - 1) as u64 => Ok((passing, below)),
        other => Err(format!(
            "inconsistent rejection immediately below {passing}: {other:?}"
        )),
    }
}
