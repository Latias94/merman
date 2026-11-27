//! Flow Step clock, ink selection, and adjacent SVG marks.

use merman_model::{
    families::flowchart::{FlowEdge, FlowchartModel},
    presentation::PresentationConfig,
};
use merman_render_core::{
    Error, Result, number_format::canonical_number as fmt, step_animation::animation_id,
    svg_output::SvgOutput, xml::escape_attr_display as escape_attr,
};

use super::FlowchartEdgeSvgContext;
use crate::{
    projection::{FlowchartEdgeKey, FlowchartRenderEdgeRef},
    style::FlowchartEdgeStylePlan,
    theme::FlowchartCompatibilityBinding,
};

pub struct FlowchartSteps {
    count: f64,
    duration: f64,
    numbers: bool,
    emitted: std::cell::Cell<bool>,
}

impl FlowchartSteps {
    pub fn new(model: &FlowchartModel, config: &PresentationConfig) -> Self {
        let flowchart = config.get("flowchart");
        Self {
            count: model.step_count(),
            duration: flowchart
                .and_then(|value| value.get("stepDuration"))
                .and_then(serde_json::Value::as_f64)
                .filter(|value| value.is_finite() && *value > 0.0)
                .unwrap_or(1.0),
            numbers: flowchart
                .and_then(|value| value.get("stepNumbers"))
                .and_then(serde_json::Value::as_bool)
                == Some(true),
            emitted: std::cell::Cell::new(false),
        }
    }

    fn timing(&self, step: f64) -> Result<(f64, f64, f64)> {
        let begin = (step - 1.0) * self.duration;
        let gap = (self.count.max(step) - 1.0) * self.duration;
        if !begin.is_finite() || !gap.is_finite() || !(gap + self.duration).is_finite() {
            return Err(Error::InvalidModel {
                message: "Flowchart step timing exceeds the finite SVG clock range".to_owned(),
            });
        }
        Ok((begin, self.duration, gap))
    }

    pub fn badges_enabled(&self) -> bool {
        self.numbers && self.count != 0.0
    }

    pub fn color_for<'a>(
        style_plan: &'a FlowchartEdgeStylePlan,
        compatibility: &'a FlowchartCompatibilityBinding,
        key: FlowchartEdgeKey,
    ) -> Result<&'a str> {
        let source = style_plan
            .edge_for(key)?
            .edge_marker_color(compatibility.look.is_hand_drawn());
        Ok(source
            .unwrap_or(&compatibility.line_color)
            .trim_start_matches("stroke:")
            .trim())
    }

    pub fn emitted(&self) -> bool {
        self.emitted.get()
    }
}

pub(super) fn write_edge_marks(
    out: &mut impl SvgOutput,
    ctx: &FlowchartEdgeSvgContext<'_>,
    FlowchartRenderEdgeRef { key, edge }: FlowchartRenderEdgeRef<'_>,
    path: &str,
) -> Result<()> {
    let Some(step) = edge.step.filter(|step| FlowEdge::is_valid_step(*step)) else {
        return Ok(());
    };
    if path.is_empty() {
        return Ok(());
    }
    ctx.work_meter
        .charge(path.len().saturating_mul(2).saturating_add(1))?;
    let (begin, duration, gap) = ctx.steps.timing(step)?;
    let edge_id = ctx
        .document_ids
        .edge(key)
        .ok_or_else(|| Error::InvalidModel {
            message: "Step edge has no document identity".to_owned(),
        })?;
    let motion = animation_id(&format!("{edge_id}-step"));
    let color = FlowchartSteps::color_for(ctx.style_plan, ctx.compatibility, key)?;
    let parked = parked_transform(path);
    let mut number = ryu_js::Buffer::new();
    let begin = number.format(begin).to_owned();
    let duration = number.format(duration).to_owned();
    let gap = number.format(gap);
    let _ = write!(
        out,
        r#"<path class="edge-step-highlight" aria-hidden="true" d="{}" fill="none" stroke="{}" stroke-width="4" stroke-linecap="round" stroke-opacity="0.6" pointer-events="none" transform="{}" data-step="{}"><animateTransform attributeName="transform" type="scale" values="1;1" dur="{duration}s" begin="{motion}.begin" fill="remove"/></path><circle class="edge-step-dot" r="4" aria-hidden="true" transform="{}" data-step="{}" fill="{}" stroke="none"><animateMotion id="{motion}" path="{}" dur="{duration}s" begin="{begin}s;{motion}.end+{gap}s" fill="remove"/><animateTransform attributeName="transform" type="scale" values="1;1" dur="{duration}s" begin="{motion}.begin" fill="remove"/></circle>"#,
        escape_attr(path),
        escape_attr(color),
        escape_attr(&parked),
        fmt(step),
        escape_attr(&parked),
        fmt(step),
        escape_attr(color),
        escape_attr(path),
    );
    out.checkpoint()?;
    ctx.steps.emitted.set(true);
    Ok(())
}

fn parked_transform(path: &str) -> String {
    match svgtypes::PathParser::from(path).next() {
        Some(Ok(svgtypes::PathSegment::MoveTo { x, y, .. })) => {
            let mut number = ryu_js::Buffer::new();
            let x = number.format(x).to_owned();
            let y = number.format(y);
            format!("translate({x},{y}) scale(0)")
        }
        _ => "scale(0)".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_timing_keeps_parallel_steps_and_gaps() {
        let steps = FlowchartSteps {
            count: 5.0,
            duration: 0.25,
            numbers: true,
            emitted: std::cell::Cell::new(false),
        };
        assert_eq!(steps.timing(3.0).unwrap(), (0.5, 0.25, 1.0));
        assert_eq!(steps.timing(5.0).unwrap(), (1.0, 0.25, 1.0));
        assert_eq!(animation_id("a-b.😀"), "step_a_2d_b_2e__1f600_");
    }
}
