//! Flowchart container colors and Agentflow's separate kind-first palette.

use super::*;

const KINDS: [&str; 7] = [
    "tool",
    "task",
    "decision",
    "input",
    "refdoc",
    "connector",
    "action",
];

fn container_slot(ordinal: usize, palette_len: usize) -> usize {
    (KINDS.len() + ordinal % palette_len.saturating_sub(KINDS.len()).max(1)) % palette_len
}

pub(super) fn container_color_slot(ctx: &FlowchartRenderCtx<'_>, id: &str) -> Option<usize> {
    let colors = &ctx.compatibility.palette;
    let ordinal = ctx.model.subgraph_color_ordinal(id)?;
    (!colors.is_empty()).then(|| {
        if ctx.diagram_type == "agentflow" {
            container_slot(ordinal, colors.len())
        } else {
            ordinal % colors.len()
        }
    })
}

pub(super) fn write_css(
    out: &mut impl SvgOutput,
    diagram_id: impl std::fmt::Display + Copy,
    binding: &crate::flowchart::FlowchartCompatibilityBinding,
) -> Result<()> {
    let _ = write!(
        out,
        "#{diagram_id} .flow-cluster rect{{fill:none;stroke-width:0.75px;}}"
    );
    out.checkpoint()?;
    let borders = &binding.palette;
    if borders.is_empty() {
        return Ok(());
    }
    let look = &binding.palette_look;
    let backgrounds =
        (!binding.palette_backgrounds.is_empty()).then_some(&binding.palette_backgrounds);
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
        out.checkpoint()?;
        let selector = format!(r##"#{diagram_id} [data-look="{look}"].node.af-kind-{kind}"##);
        let _ = write!(
            out,
            "{selector} rect,{selector} path,{selector} polygon{{{}}}",
            declarations(slot)
        );
    }
    for ordinal in 0..borders.len().saturating_sub(KINDS.len()).max(1) {
        out.checkpoint()?;
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
    out.checkpoint()
}

/// Port of flowchart/styles.ts genColor. Shape attributes are stamped using this same
/// border palette length; inline author styles retain their normal CSS precedence.
pub(super) fn write_flowchart_container_css(
    out: &mut impl SvgOutput,
    diagram_id: impl std::fmt::Display + Copy,
    binding: &crate::flowchart::FlowchartCompatibilityBinding,
) -> Result<()> {
    let borders = &binding.palette;
    if borders.is_empty() {
        return Ok(());
    }
    let look = &binding.palette_look;
    let backgrounds =
        (!binding.palette_backgrounds.is_empty()).then_some(&binding.palette_backgrounds);
    for (slot, border) in borders.iter().enumerate() {
        out.checkpoint()?;
        let border = border
            .as_str()
            .map(|value| value.trim().to_owned())
            .unwrap_or_else(|| border.to_string());
        let background = backgrounds.map(|values| {
            let value = &values[slot % values.len()];
            value
                .as_str()
                .map(|value| value.trim().to_owned())
                .unwrap_or_else(|| value.to_string())
        });
        let fill = background
            .as_ref()
            .map(|value| format!("fill:{value};"))
            .unwrap_or_default();
        let prefix =
            format!(r##"#{diagram_id} [data-look="{look}"][data-color-id="color-{slot}"]"##);
        let collapsed =
            |suffix: &str| format!("{prefix}.node {suffix},{prefix}.rough-node {suffix}");
        let lane = |suffix: &str| {
            format!(
                "{prefix}.swimlane.cluster .swimlane-title{suffix},{prefix}.swimlane.cluster .swimlane-body{suffix}"
            )
        };
        let _ = write!(
            out,
            "{prefix}.cluster:not(.swimlane) rect{{stroke:{border};{fill}}}{prefix}.cluster:not(.swimlane) path{{stroke:{border};{fill}}}{prefix}.swimlane.cluster rect.swimlane-title,{prefix}.swimlane.cluster rect.swimlane-body{{stroke:{border};{fill}}}{}{{stroke:{border};}}",
            lane(" path:nth-of-type(2)")
        );
        if let Some(background) = background {
            let _ = write!(
                out,
                "{}{{stroke:{background};}}",
                lane(" path:first-of-type")
            );
        }
        let _ = write!(
            out,
            "{},{}{{stroke:{border};{fill}}}{}{{fill:{border};}}{}{{stroke:{border};}}",
            collapsed(".collapsed-group"),
            collapsed(".collapsed-group path"),
            collapsed(".collapsed-indicator"),
            collapsed(".collapsed-separator")
        );
    }
    out.checkpoint()
}
