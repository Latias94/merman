use super::*;
use kurbo::{ParamCurve, ParamCurveArclen};
use merman_markup::theme_color::{ColorChannel, ThemeColor};

const BADGE_RADIUS: f64 = 9.0;
const BADGE_FRACTIONS: [f64; 9] = [0.25, 0.35, 0.15, 0.65, 0.75, 0.5, 0.85, 0.1, 0.9];

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
        let flowchart = config.as_value().get("flowchart");
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

    pub(super) fn write_style(&self, out: &mut impl SvgOutput, diagram_id: SvgDiagramId<'_>) {
        if !self.emitted.get() {
            return;
        }
        let scope = super::css::FlowchartCssSelectorDiagramId::new(diagram_id);
        let _ = write!(
            out,
            r#"<style class="edge-step-style">@media (prefers-reduced-motion: reduce) {{ #{scope} .edge-step-dot, #{scope} .edge-step-highlight {{ display: none; }} }}</style>"#,
        );
    }
}

fn animation_id(value: &str) -> String {
    let mut id = String::from("step_");
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            id.push(character);
        } else {
            let _ = write!(id, "_{:x}_", character as u32);
        }
    }
    id
}

fn step_color<'a>(
    ctx: &'a FlowchartRenderCtx<'_>,
    key: crate::flowchart::FlowchartEdgeKey,
    typed_stroke: Option<&'a str>,
) -> crate::Result<&'a str> {
    let source = ctx
        .edge_style_plan
        .edge_for(key)?
        .edge_marker_color(ctx.compatibility.look.is_hand_drawn());
    Ok(typed_stroke
        .or(source)
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
    typed_stroke: Option<&str>,
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
    let color = step_color(ctx, key, typed_stroke)?;
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

#[derive(Clone, Copy)]
struct Obstacle {
    x: f64,
    y: f64,
    half_width: f64,
    half_height: f64,
}

impl Obstacle {
    fn square(point: kurbo::Point, half: f64) -> Self {
        Self {
            x: point.x,
            y: point.y,
            half_width: half,
            half_height: half,
        }
    }

    fn room(self, point: kurbo::Point) -> f64 {
        let dx = ((point.x - self.x).abs() - self.half_width).max(0.0);
        let dy = ((point.y - self.y).abs() - self.half_height).max(0.0);
        dx.hypot(dy) - BADGE_RADIUS
    }
}

fn arrow_obstacle(tip: kurbo::Point, from: kurbo::Point) -> Obstacle {
    let delta = tip - from;
    let length = delta.hypot();
    if length == 0.0 {
        return Obstacle::square(tip, 12.0);
    }
    let along = delta / length;
    let center = tip - along * 6.0;
    let half = (along.x.abs() + along.y.abs()) * 6.0;
    Obstacle {
        x: center.x,
        y: center.y,
        half_width: half,
        half_height: half,
    }
}

struct PathSampler {
    segments: Vec<(kurbo::PathSeg, f64)>,
    length: f64,
    fallback: Option<kurbo::Point>,
}

impl PathSampler {
    fn new(path: &str, points: &[crate::model::LayoutPoint]) -> Self {
        let segments = kurbo::BezPath::from_svg(path)
            .ok()
            .map(|path| {
                path.segments()
                    .map(|segment| (segment, segment.arclen(1e-6)))
                    .collect::<Vec<_>>()
            })
            .filter(|segments| segments.iter().any(|(_, length)| *length > 0.0))
            .unwrap_or_else(|| {
                points
                    .windows(2)
                    .map(|pair| {
                        let line = kurbo::Line::new((pair[0].x, pair[0].y), (pair[1].x, pair[1].y));
                        (kurbo::PathSeg::Line(line), line.arclen(1e-6))
                    })
                    .collect()
            });
        let length = segments.iter().map(|(_, length)| length).sum();
        let fallback = points
            .iter()
            .find(|point| point.x.is_finite() && point.y.is_finite())
            .map(|point| kurbo::Point::new(point.x, point.y));
        Self {
            segments,
            length,
            fallback,
        }
    }

    fn sample(&self, fraction: f64) -> Option<kurbo::Point> {
        let mut remaining = self.length * fraction;
        for (segment, length) in &self.segments {
            if remaining <= *length && *length > 0.0 {
                return Some(segment.eval(segment.inv_arclen(remaining, 1e-6)));
            }
            remaining -= length;
        }
        self.segments
            .last()
            .map(|(segment, _)| segment.eval(1.0))
            .or(self.fallback)
    }
}

fn choose_position(
    sampler: &PathSampler,
    obstacles: &[Obstacle],
    work: &crate::resources::OperationWorkMeter,
) -> crate::Result<Option<kurbo::Point>> {
    work.charge(sampler.segments.len().saturating_mul(63))?;
    let mut candidates = Vec::with_capacity(27);
    let centers = BADGE_FRACTIONS.map(|fraction| sampler.sample(fraction));
    for point in centers {
        candidates.extend(point);
    }
    for (fraction, point) in BADGE_FRACTIONS.into_iter().zip(centers) {
        if let (Some(point), Some(before), Some(after)) = (
            point,
            sampler.sample((fraction - 0.01).max(0.0)),
            sampler.sample((fraction + 0.01).min(1.0)),
        ) {
            let delta = after - before;
            let length = delta.hypot();
            if length > 0.0 {
                let offset = kurbo::Vec2::new(-delta.y, delta.x) * (14.0 / length);
                candidates.extend([point + offset, point - offset]);
            }
        }
    }
    let mut best = None;
    let mut best_room = f64::NEG_INFINITY;
    for point in candidates
        .into_iter()
        .filter(|point| point.x.is_finite() && point.y.is_finite())
    {
        work.charge(obstacles.len().max(1))?;
        let room = obstacles
            .iter()
            .map(|obstacle| obstacle.room(point))
            .fold(f64::INFINITY, f64::min);
        if room >= 2.0 {
            return Ok(Some(point));
        }
        if room > best_room {
            best_room = room;
            best = Some(point);
        }
    }
    Ok(best)
}

fn contrast(first: &str, second: &str) -> f64 {
    let luminance = |color| {
        ThemeColor::parse(color).ok().map(|color| {
            [ColorChannel::Red, ColorChannel::Green, ColorChannel::Blue]
                .into_iter()
                .zip([0.2126, 0.7152, 0.0722])
                .map(|(channel, weight)| {
                    let srgb = color.channel(channel) / 255.0;
                    weight
                        * if srgb <= 0.03928 {
                            srgb / 12.92
                        } else {
                            ((srgb + 0.055) / 1.055).powf(2.4)
                        }
                })
                .sum::<f64>()
        })
    };
    match (luminance(first), luminance(second)) {
        (Some(first), Some(second)) => (first.max(second) + 0.05) / (first.min(second) + 0.05),
        _ => 0.0,
    }
}

fn text_color<'a>(fill: &str, candidates: impl IntoIterator<Item = &'a str>) -> &'a str {
    let mut best = "#ffffff";
    let mut best_ratio = -1.0;
    for color in candidates.into_iter().chain(["#ffffff", "#000000"]) {
        if color.is_empty() {
            continue;
        }
        let ratio = contrast(fill, color);
        if ratio >= 4.5 {
            return color;
        }
        if ratio > best_ratio {
            best = color;
            best_ratio = ratio;
        }
    }
    best
}

struct Badge {
    point: kurbo::Point,
    origin: kurbo::Vec2,
    step: f64,
    color: String,
}

#[derive(Default)]
pub(super) struct StepBadges {
    badges: Vec<Badge>,
    by_root: FxHashMap<String, Vec<usize>>,
}

impl StepBadges {
    pub(super) fn prepare(
        ctx: &FlowchartRenderCtx<'_>,
        hierarchy: &FlowchartHierarchyPlan<'_>,
        cache: &FxHashMap<crate::flowchart::FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
        labels: &super::paint::FlowchartEdgeLabels,
    ) -> crate::Result<Self> {
        let mut result = Self::default();
        if !ctx.steps.numbers || ctx.steps.count == 0.0 {
            return Ok(result);
        }
        let mut obstacles = Vec::new();
        for id in hierarchy.rendered_node_ids() {
            ctx.work_meter.charge(1)?;
            if let Some(node) = ctx.layout_nodes_by_id.get(id) {
                let offsets = hierarchy
                    .effective_parent(id)
                    .and_then(|root| hierarchy.root_offsets(root));
                let shift_y =
                    offsets.map_or(0.0, |offset| offset.abs_top_transform - offset.origin_y);
                obstacles.push(Obstacle {
                    x: node.x,
                    y: node.y + shift_y,
                    half_width: node.width / 2.0,
                    half_height: node.height / 2.0,
                });
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
                .and_then(|edge| edge.label.as_ref())
                && let Some(point) = labels.position(edge.key)
            {
                obstacles.push(Obstacle {
                    x: point.x,
                    y: point.y + entry.abs_top_transform - entry.origin_y,
                    half_width: label.width / 2.0,
                    half_height: label.height / 2.0,
                });
            }
            let points = &entry.geom.data_points;
            let global = |point: &crate::model::LayoutPoint| {
                kurbo::Point::new(point.x + entry.origin_x, point.y + entry.abs_top_transform)
            };
            if points.len() >= 2 {
                let (start, end) =
                    super::edge_geom::arrow_types_for_edge(edge.edge.edge_type.as_deref());
                if start.is_some_and(|marker| marker != "none") {
                    obstacles.push(arrow_obstacle(global(&points[0]), global(&points[1])));
                }
                if end.is_some_and(|marker| marker != "none") {
                    obstacles.push(arrow_obstacle(
                        global(&points[points.len() - 1]),
                        global(&points[points.len() - 2]),
                    ));
                }
            }
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
            ctx.work_meter
                .charge(geom.d.len().saturating_add(geom.data_points.len()))?;
            let sampler = PathSampler::new(
                geom.emitted_d_for_label.as_deref().unwrap_or(&geom.d),
                &geom.data_points,
            );
            ctx.work_meter.charge(obstacles.len())?;
            let local: Vec<_> = obstacles
                .iter()
                .map(|obstacle| Obstacle {
                    x: obstacle.x - entry.origin_x,
                    y: obstacle.y - entry.abs_top_transform,
                    ..*obstacle
                })
                .collect();
            let Some(point) = choose_position(&sampler, &local, ctx.work_meter)? else {
                continue;
            };
            let point = point + kurbo::Vec2::new(entry.origin_x, entry.abs_top_transform);
            let precedence = crate::flowchart::FlowchartFacetPrecedence::new(
                ctx.edge_style_plan.emitted_source_stroke_status_for(
                    edge.key,
                    ctx.compatibility.look.is_hand_drawn(),
                )?,
                ctx.edge_stroke_config_override,
            );
            let color = step_color(ctx, edge.key, ctx.edge_theme.stroke_value(precedence, true))?
                .to_owned();
            result
                .by_root
                .entry(hierarchy.edge_root(edge.key)?.unwrap_or("").to_owned())
                .or_default()
                .push(result.badges.len());
            result.badges.push(Badge {
                point,
                step,
                color,
                origin: kurbo::Vec2::new(entry.origin_x, entry.abs_top_transform),
            });
            obstacles.push(Obstacle::square(point, BADGE_RADIUS));
        }
        Ok(result)
    }

    pub(super) fn include_bounds(
        &self,
        min_x: &mut f64,
        min_y: &mut f64,
        max_x: &mut f64,
        max_y: &mut f64,
    ) {
        for badge in &self.badges {
            *min_x = min_x.min(badge.point.x - BADGE_RADIUS);
            *min_y = min_y.min(badge.point.y - BADGE_RADIUS);
            *max_x = max_x.max(badge.point.x + BADGE_RADIUS);
            *max_y = max_y.max(badge.point.y + BADGE_RADIUS);
        }
    }

    pub(super) fn write_root(
        &self,
        out: &mut impl SvgOutput,
        ctx: &FlowchartRenderCtx<'_>,
        root: Option<&str>,
    ) -> crate::Result<()> {
        let Some(indices) = self.by_root.get(root.unwrap_or("")) else {
            return Ok(());
        };
        out.push_str(r#"<g class="edge-step-badges">"#);
        let background = ctx
            .config
            .get_str("themeVariables.background")
            .unwrap_or_default();
        let primary = ctx
            .config
            .get_str("themeVariables.primaryTextColor")
            .unwrap_or_default();
        for index in indices {
            ctx.checkpoint_emit()?;
            let badge = &self.badges[*index];
            let point = badge.point - badge.origin;
            let _ = write!(
                out,
                r#"<g class="edge-step-badge" transform="translate({},{})" pointer-events="none" data-step="{}"><circle r="9" fill="{}"/><text text-anchor="middle" dominant-baseline="central" font-size="11" fill="{}">{}</text></g>"#,
                fmt(point.x),
                fmt(point.y),
                fmt(badge.step),
                escape_attr(&badge.color),
                escape_attr(text_color(&badge.color, [background, primary])),
                fmt(badge.step)
            );
        }
        out.push_str("</g>");
        out.checkpoint()
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
        assert_eq!(text_color("#ffffff", ["#ffffff", "#333333"]), "#333333");
        assert_eq!(text_color("#333333", ["#ffffff", "#333333"]), "#ffffff");
    }

    #[test]
    fn badge_sampler_uses_curve_length_and_clears_prior_badges() {
        let sampler = PathSampler::new("M0,0L100,0", &[]);
        let work = crate::resources::OperationWorkMeter::new(
            crate::resources::RenderResourcePolicy::unbounded_for_trusted_input(),
        );
        let first = choose_position(&sampler, &[], &work).unwrap().unwrap();
        let second = choose_position(&sampler, &[Obstacle::square(first, 9.0)], &work)
            .unwrap()
            .unwrap();
        assert_eq!(first, kurbo::Point::new(25.0, 0.0));
        assert!(first.distance(second) >= 20.0);
        let singleton = PathSampler::new("", &[crate::model::LayoutPoint { x: 1.0, y: 2.0 }]);
        assert_eq!(singleton.sample(0.25), Some(kurbo::Point::new(1.0, 2.0)));
    }
}
