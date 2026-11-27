use super::*;
use merman_render_core::step_animation::animation_id;
use merman_render_flow::graph::{StepBadgePlacement, StepBadges};

pub(super) struct FlowchartSteps {
    count: f64,
    duration: f64,
    numbers: bool,
    emitted: std::cell::Cell<bool>,
}

impl FlowchartSteps {
    pub(super) fn new(
        model: &crate::flowchart::FlowchartModel,
        config: &merman_model::presentation::PresentationConfig,
    ) -> Self {
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

    fn timing(&self, step: f64) -> crate::Result<(f64, f64, f64)> {
        let begin = (step - 1.0) * self.duration;
        let gap = (self.count.max(step) - 1.0) * self.duration;
        if !begin.is_finite() || !gap.is_finite() || !(gap + self.duration).is_finite() {
            return Err(crate::Error::InvalidModel {
                message: "Flowchart step timing exceeds the finite SVG clock range".to_owned(),
            });
        }
        Ok((begin, self.duration, gap))
    }

    pub(super) fn emitted(&self) -> bool {
        self.emitted.get()
    }
}

fn step_color<'a>(
    ctx: &'a FlowchartRenderCtx<'_>,
    key: crate::flowchart::FlowchartEdgeKey,
) -> crate::Result<&'a str> {
    let source = ctx
        .edge_style_plan
        .edge_for(key)?
        .edge_marker_color(ctx.compatibility.look.is_hand_drawn());
    Ok(source
        .unwrap_or(&ctx.compatibility.line_color)
        .trim_start_matches("stroke:")
        .trim())
}

pub(super) fn write_edge_marks(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    key: crate::flowchart::FlowchartEdgeKey,
    edge: &crate::flowchart::FlowEdge,
    path: &str,
) -> crate::Result<()> {
    let Some(step) = edge
        .step
        .filter(|step| crate::flowchart::FlowEdge::is_valid_step(*step))
    else {
        return Ok(());
    };
    if path.is_empty() {
        return Ok(());
    }
    ctx.work_meter
        .charge(path.len().saturating_mul(2).saturating_add(1))?;
    let (begin, duration, gap) = ctx.steps.timing(step)?;
    let edge_id = ctx
        .document_ids()
        .edge(key)
        .ok_or_else(|| crate::Error::InvalidModel {
            message: "Step edge has no document identity".to_owned(),
        })?;
    let motion = animation_id(&format!("{edge_id}-step"));
    let color = step_color(ctx, key)?;
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

pub(super) fn prepare_step_badges<'a>(
    ctx: &FlowchartRenderCtx<'a>,
    hierarchy: &FlowchartHierarchyPlan<'_>,
    cache: &FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    labels: &super::paint::FlowchartEdgeLabels,
) -> crate::Result<StepBadges<'a>> {
    let mut placement = StepBadgePlacement::new(ctx.config, ctx.work_meter);
    if !ctx.steps.numbers || ctx.steps.count == 0.0 {
        return Ok(placement.finish());
    }
    for id in hierarchy.rendered_node_ids() {
        ctx.work_meter.charge(1)?;
        if let Some(node) = ctx.layout_nodes_by_id.get(id) {
            let offsets = hierarchy
                .effective_parent(id)
                .and_then(|root| hierarchy.root_offsets(root));
            let shift_y = offsets.map_or(0.0, |offset| offset.abs_top_transform - offset.origin_y);
            placement.push_box_obstacle(
                kurbo::Point::new(node.x, node.y + shift_y),
                kurbo::Size::new(node.width, node.height),
            );
        }
    }
    for edge in hierarchy.ordered_edges() {
        ctx.work_meter.charge(1)?;
        let Some(entry) = cache.get(&edge.key) else {
            continue;
        };
        if let Some(label) = ctx
            .layout_edges_by_key
            .get(&edge.key)
            .and_then(|edge| edge.label())
            && let Some(point) = labels.position(edge.key)
        {
            placement.push_box_obstacle(
                kurbo::Point::new(point.x, point.y + entry.abs_top_transform - entry.origin_y),
                kurbo::Size::new(label.width, label.height),
            );
        }
        placement.push_arrow_obstacles(
            &entry.geom.data_points,
            edge.edge.edge_type.as_deref(),
            kurbo::Vec2::new(entry.origin_x, entry.abs_top_transform),
        );
    }
    for edge in hierarchy.ordered_edges() {
        ctx.work_meter.charge(1)?;
        let Some(step) = edge
            .edge
            .step
            .filter(|step| crate::flowchart::FlowEdge::is_valid_step(*step))
        else {
            continue;
        };
        let Some(entry) = cache.get(&edge.key) else {
            continue;
        };
        let geom = &entry.geom;
        let origin = kurbo::Vec2::new(entry.origin_x, entry.abs_top_transform);
        let Some(point) = placement.place(
            &geom.d,
            geom.emitted_d_for_label.as_deref(),
            &geom.data_points,
            origin,
        )?
        else {
            continue;
        };
        let color = step_color(ctx, edge.key)?.to_owned();
        let root = hierarchy.edge_root(edge.key)?;
        placement.push_badge(point, origin, step, color, root);
    }
    Ok(placement.finish())
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
