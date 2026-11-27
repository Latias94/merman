//! Add line hops to the painted route without rebuilding its bends or terminals.

use super::*;
use crate::flowchart::{FlowchartEdgeKey, FlowchartEdgeTransportPlan};
use crate::model::LayoutPoint;
use crate::svg::parity::flowchart::render_input::FlowchartRenderEdge;
use crate::svg::parity::line_hops::{
    LineHopConfig, LineHopCrossing, LineHopEdge, LineHopStyle, curve_supports_line_hops,
    find_edge_intersections_where, painted_line_hop_intervals,
};
use kurbo::{Point, Vec2};
use svgtypes::{PathParser, PathSegment};

struct Command {
    offset: usize,
    end: Point,
    line: bool,
}

struct Shaft {
    commands: Vec<Command>,
    points: Vec<LayoutPoint>,
}

impl Shaft {
    fn new(entry: &FlowchartEdgePathCacheEntry) -> Option<Self> {
        let d = entry.geom.d.as_str();
        // The canonical writer spells out each absolute command. Keep its source offsets so
        // a jump can replace a line without reserializing any curves, arcs, or marker ends.
        let mut offsets =
            d.match_indices(|ch: char| ch.is_ascii_alphabetic() && !matches!(ch, 'e' | 'E'));
        let mut commands: Vec<Command> = Vec::new();
        for segment in PathParser::from(d) {
            let segment = segment.ok()?;
            let (offset, _) = offsets.next()?;
            let end = match segment {
                PathSegment::MoveTo { abs: true, x, y }
                | PathSegment::LineTo { abs: true, x, y }
                | PathSegment::CurveTo {
                    abs: true, x, y, ..
                }
                | PathSegment::EllipticalArc {
                    abs: true, x, y, ..
                }
                | PathSegment::Quadratic {
                    abs: true, x, y, ..
                } => Point::new(x, y),
                _ => return None,
            };
            let line = matches!(segment, PathSegment::LineTo { .. });
            if line
                && let [.., a, b] = commands.as_mut_slice()
                && b.line
                && same_axis_direction(b.end - a.end, end - b.end)
            {
                b.end = end;
            } else {
                commands.push(Command { offset, end, line });
            }
        }
        if offsets.next().is_some() || commands.len() < 2 {
            return None;
        }
        let offset = Vec2::new(entry.origin_x, entry.abs_top_transform);
        let points = commands
            .iter()
            .map(|command| {
                let point = command.end + offset;
                LayoutPoint {
                    x: point.x,
                    y: point.y,
                }
            })
            .collect();
        Some(Self { commands, points })
    }

    fn is_axis_segment(&self, index: usize) -> bool {
        if !self.commands[index + 1].line {
            return false;
        }
        let (a, b) = (&self.points[index], &self.points[index + 1]);
        (a.x == b.x && a.y != b.y) || (a.x != b.x && a.y == b.y)
    }

    fn hopped_path(
        &self,
        original: &str,
        by_segment: &FxHashMap<usize, Vec<LineHopCrossing<'_>>>,
        config: LineHopConfig,
    ) -> Option<String> {
        let mut output = String::new();
        let mut copied = 0;
        for (index, commands) in self.commands.windows(2).enumerate() {
            let Some(crossings) = by_segment.get(&index) else {
                continue;
            };
            let start = commands[0].end;
            let command = &commands[1];
            let length = (command.end - start).hypot();
            let intervals = painted_line_hop_intervals(length, crossings, config.jump_radius);
            if intervals.is_empty() {
                continue;
            }
            output.push_str(&original[copied..command.offset]);
            write_line_hops(
                &mut output,
                start,
                command.end,
                length,
                &intervals,
                config.jump_style,
            );
            let _ = write!(
                output,
                "L{},{}",
                fmt_display(command.end.x),
                fmt_display(command.end.y),
            );
            copied = self
                .commands
                .get(index + 2)
                .map_or(original.len(), |next| next.offset);
        }
        if copied == 0 {
            return None;
        }
        output.push_str(&original[copied..]);
        Some(output)
    }
}

pub(in crate::svg::parity::flowchart) fn apply_canonical_line_hops(
    cache: &mut FxHashMap<FlowchartEdgeKey, FlowchartEdgePathCacheEntry>,
    render_edges: &[FlowchartRenderEdge<'_>],
    ctx: &FlowchartRenderCtx<'_>,
    preserve_dash_mask_length: impl Fn(&FlowchartRenderEdge<'_>) -> bool,
) -> Result<()> {
    let Some(config) = super::line_hop_config(ctx.config, "swimlane") else {
        return Ok(());
    };
    if render_edges.len() < 2 {
        return Ok(());
    }
    ctx.work_meter.charge(render_edges.len())?;
    let transport = FlowchartEdgeTransportPlan::for_keyed_edges(
        render_edges
            .iter()
            .map(|edge| (edge.key, edge.edge.as_ref())),
    );
    let theme_sketch = super::super::paint::uses_theme_sketch(ctx);
    let mut shafts = Vec::new();
    for edge in render_edges {
        if edge.edge.visibility == merman_model::families::flowchart::FlowEdgeVisibility::Invisible
            || (theme_sketch && !super::super::paint::has_authored_curve(ctx, edge.key))
        {
            continue;
        }
        if let Some((entry, id)) = cache.get(&edge.key).zip(transport.id(edge.key)) {
            ctx.work_meter.charge(entry.geom.d.len())?;
            if let Some(shaft) = Shaft::new(entry) {
                shafts.push((edge, id, shaft));
            }
        }
    }
    let edges: Vec<_> = shafts
        .iter()
        .map(|(_, id, shaft)| LineHopEdge {
            id,
            points: &shaft.points,
            curve: None,
            arrow_type_start: None,
            arrow_type_end: None,
        })
        .collect();
    let crossings = find_edge_intersections_where(&edges, ctx.work_meter, |edge, segment| {
        shafts[edge].2.is_axis_segment(segment)
    })?;
    let levels = crossings.len().checked_ilog2().unwrap_or(0) as usize + 1;
    ctx.work_meter
        .charge(crossings.len().saturating_mul(levels))?;
    let mut by_edge: FxHashMap<_, FxHashMap<_, Vec<_>>> = FxHashMap::default();
    for crossing in crossings {
        by_edge
            .entry(crossing.jump_edge_id)
            .or_default()
            .entry(crossing.segment_index)
            .or_default()
            .push(crossing);
    }
    for (edge, id, shaft) in &shafts {
        if !curve_supports_line_hops(edge.edge.interpolate.as_deref()) {
            continue;
        }
        let Some(by_segment) = by_edge.get(id) else {
            continue;
        };
        ctx.work_meter.charge(shaft.points.len())?;
        if let Some(entry) = cache.get_mut(&edge.key)
            && let Some(path) = shaft.hopped_path(&entry.geom.d, by_segment, config)
        {
            if preserve_dash_mask_length(edge) {
                entry.geom.pre_hop_dash_mask_length = entry.geom.centerline_length();
            }
            entry.geom.d = path;
            entry.geom.neo_length = None;
            entry.geom.pb = svg_path_bounds_from_d(&entry.geom.d);
            entry.geom.line_hop_applied = true;
        }
    }
    Ok(())
}

fn same_axis_direction(a: Vec2, b: Vec2) -> bool {
    ((a.x == 0.0 && b.x == 0.0) || (a.y == 0.0 && b.y == 0.0)) && a.dot(b) > 0.0
}

fn write_line_hops(
    out: &mut String,
    start: Point,
    end: Point,
    length: f64,
    intervals: &[(f64, f64)],
    style: LineHopStyle,
) {
    let delta = end - start;
    let unit = delta / length;
    let sweep = u8::from(if delta.x == 0.0 {
        delta.y > 0.0
    } else {
        delta.x > 0.0
    });
    for &(center, radius) in intervals {
        let before = start + unit * (center - radius);
        let after = start + unit * (center + radius);
        let _ = write!(out, "L{},{}", fmt_display(before.x), fmt_display(before.y));
        match style {
            LineHopStyle::Arc => {
                let _ = write!(
                    out,
                    "A{0},{0} 0 0 {sweep} {1},{2}",
                    fmt_display(radius),
                    fmt_display(after.x),
                    fmt_display(after.y)
                );
            }
            LineHopStyle::Gap => {
                let _ = write!(out, "M{},{}", fmt_display(after.x), fmt_display(after.y));
            }
        }
    }
}
