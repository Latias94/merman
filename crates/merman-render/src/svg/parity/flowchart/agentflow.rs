//! Agentflow's kind palette and declaration-ordered container colors.

use super::*;
use serde_json::Value;

const KINDS: [&str; 7] = [
    "tool",
    "task",
    "decision",
    "input",
    "refdoc",
    "connector",
    "action",
];

fn palette(config: &Value) -> &[Value] {
    if !matches!(
        config.get("theme").and_then(Value::as_str),
        Some("redux-color" | "redux-dark-color")
    ) {
        return &[];
    }
    config
        .pointer("/themeVariables/borderColorArray")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn container_slot(ordinal: usize, palette_len: usize) -> usize {
    (KINDS.len() + ordinal % palette_len.saturating_sub(KINDS.len()).max(1)) % palette_len
}

pub(super) fn container_color_slot(ctx: &FlowchartRenderCtx<'_>, id: &str) -> Option<usize> {
    if ctx.diagram_type != "agentflow" {
        return None;
    }
    let colors = palette(ctx.config.as_value());
    let ordinal = ctx.model.subgraph_color_ordinal(id)?;
    (!colors.is_empty()).then(|| container_slot(ordinal, colors.len()))
}

pub(super) fn css(
    diagram_id: SvgDiagramId<'_>,
    config: &Value,
    emit: FlowchartEmitCheckpoint<'_>,
) -> Result<String> {
    let mut out = format!("#{diagram_id} .flow-cluster rect{{fill:none;stroke-width:0.75px;}}");
    let borders = palette(config);
    if borders.is_empty() {
        return Ok(out);
    }
    let look = config
        .get("look")
        .and_then(Value::as_str)
        .filter(|look| {
            !look.is_empty()
                && look
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        })
        .unwrap_or("classic");
    let backgrounds = config
        .pointer("/themeVariables/bkgColorArray")
        .and_then(Value::as_array)
        .filter(|values| !values.is_empty());
    let declarations = |slot: usize| {
        let color = &borders[slot % borders.len()];
        let border = color
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| color.to_string());
        let fill = backgrounds
            .map(|values| {
                let color = &values[slot % values.len()];
                format!(
                    "fill:{};",
                    color
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| color.to_string())
                )
            })
            .unwrap_or_default();
        format!("stroke:{border};{fill}")
    };
    for (slot, kind) in KINDS.iter().enumerate() {
        emit.checkpoint()?;
        let selector = format!(r##"#{diagram_id} [data-look="{look}"].node.af-kind-{kind}"##);
        let _ = write!(
            out,
            "{selector} rect,{selector} path,{selector} polygon{{{}}}",
            declarations(slot)
        );
    }
    for ordinal in 0..borders.len().saturating_sub(KINDS.len()).max(1) {
        emit.checkpoint()?;
        let slot = container_slot(ordinal, borders.len());
        let expanded = format!(
            r##"#{diagram_id} [data-look="{look}"][data-color-id="color-{slot}"].cluster"##
        );
        let collapsed =
            format!(r##"#{diagram_id} [data-look="{look}"][data-color-id="color-{slot}"].node"##);
        let _ = write!(
            out,
            "{expanded} rect,{collapsed} rect,{expanded} path,{collapsed} path{{{}}}",
            declarations(slot)
        );
    }
    Ok(out)
}
