//! Flowchart Step badge placement and balanced SVG emission.

use kurbo::{ParamCurve, ParamCurveArclen, Point, Size, Vec2};
use merman_markup::theme_color::{ColorChannel, ThemeColor};
use merman_model::{OperationPhase, presentation::PresentationConfig};
use merman_render_core::{
    LayoutPoint, Result, number_format::canonical_number as fmt, resources::OperationWorkMeter,
    svg_output::SvgOutput, xml::escape_attr_display,
};
use rustc_hash::FxHashMap;

use crate::edge_svg::arrow_types_for_edge;

const BADGE_RADIUS: f64 = 9.0;
const BADGE_FRACTIONS: [f64; 9] = [0.25, 0.35, 0.15, 0.65, 0.75, 0.5, 0.85, 0.1, 0.9];

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
    fn new(path: &str, points: &[LayoutPoint]) -> Self {
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
    work: &OperationWorkMeter,
) -> Result<Option<kurbo::Point>> {
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

/// Places numbered Step badges using the original graph, path, and paint facts.
/// All inputs belong to the same prepared rendering operation.
pub struct StepBadgePlacement<'a> {
    obstacles: Vec<Obstacle>,
    badges: StepBadges<'a>,
}

impl<'a> StepBadgePlacement<'a> {
    pub fn new(config: &'a PresentationConfig, work_meter: &'a OperationWorkMeter) -> Self {
        Self {
            obstacles: Vec::new(),
            badges: StepBadges {
                badges: Vec::new(),
                by_root: FxHashMap::default(),
                config,
                work_meter,
            },
        }
    }

    pub fn push_box_obstacle(&mut self, center: Point, size: Size) {
        self.obstacles.push(Obstacle {
            x: center.x,
            y: center.y,
            half_width: size.width / 2.0,
            half_height: size.height / 2.0,
        });
    }

    pub fn push_arrow_obstacles(
        &mut self,
        points: &[LayoutPoint],
        edge_type: Option<&str>,
        origin: Vec2,
    ) {
        if points.len() < 2 {
            return;
        }
        let global = |point: &LayoutPoint| Point::new(point.x + origin.x, point.y + origin.y);
        let (start, end) = arrow_types_for_edge(edge_type);
        if start.is_some_and(|marker| marker != "none") {
            self.obstacles
                .push(arrow_obstacle(global(&points[0]), global(&points[1])));
        }
        if end.is_some_and(|marker| marker != "none") {
            self.obstacles.push(arrow_obstacle(
                global(&points[points.len() - 1]),
                global(&points[points.len() - 2]),
            ));
        }
    }

    /// Charges the logical path before sampling the visible label path and testing obstacles.
    pub fn place(
        &self,
        path: &str,
        emitted_label_path: Option<&str>,
        points: &[LayoutPoint],
        origin: Vec2,
    ) -> Result<Option<Point>> {
        let work = self.badges.work_meter;
        work.charge(path.len().saturating_add(points.len()))?;
        let sampler = PathSampler::new(emitted_label_path.unwrap_or(path), points);
        work.charge(self.obstacles.len())?;
        let local: Vec<_> = self
            .obstacles
            .iter()
            .map(|obstacle| Obstacle {
                x: obstacle.x - origin.x,
                y: obstacle.y - origin.y,
                ..*obstacle
            })
            .collect();
        Ok(choose_position(&sampler, &local, work)?.map(|point| point + origin))
    }

    pub fn push_badge(
        &mut self,
        point: Point,
        origin: Vec2,
        step: f64,
        color: String,
        root: Option<&str>,
    ) {
        self.badges
            .by_root
            .entry(root.unwrap_or("").to_owned())
            .or_default()
            .push(self.badges.badges.len());
        self.badges.badges.push(Badge {
            point,
            origin,
            step,
            color,
        });
        self.obstacles.push(Obstacle::square(point, BADGE_RADIUS));
    }

    pub fn finish(self) -> StepBadges<'a> {
        self.badges
    }
}

/// Prepared badges retain their root ownership and draw source order.
pub struct StepBadges<'a> {
    badges: Vec<Badge>,
    by_root: FxHashMap<String, Vec<usize>>,
    config: &'a PresentationConfig,
    work_meter: &'a OperationWorkMeter,
}

impl StepBadges<'_> {
    pub fn include_bounds(
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

    pub(super) fn write_root(&self, out: &mut impl SvgOutput, root: Option<&str>) -> Result<()> {
        let Some(indices) = self.by_root.get(root.unwrap_or("")) else {
            return Ok(());
        };
        out.push_str(r#"<g class="edge-step-badges">"#);
        let background = self
            .config
            .get_str("themeVariables.background")
            .unwrap_or_default();
        let primary = self
            .config
            .get_str("themeVariables.primaryTextColor")
            .unwrap_or_default();
        for index in indices {
            self.work_meter.checkpoint(OperationPhase::Emit)?;
            let badge = &self.badges[*index];
            let point = badge.point - badge.origin;
            let _ = write!(
                out,
                r#"<g class="edge-step-badge" transform="translate({},{})" pointer-events="none" data-step="{}"><circle r="9" fill="{}"/><text text-anchor="middle" dominant-baseline="central" font-size="11" fill="{}">{}</text></g>"#,
                fmt(point.x),
                fmt(point.y),
                fmt(badge.step),
                escape_attr_display(&badge.color),
                escape_attr_display(text_color(&badge.color, [background, primary])),
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
    use merman_render_core::resources::RenderResourcePolicy;

    #[test]
    fn badge_text_uses_the_first_legible_color() {
        assert_eq!(text_color("#ffffff", ["#ffffff", "#333333"]), "#333333");
        assert_eq!(text_color("#333333", ["#ffffff", "#333333"]), "#ffffff");
    }

    #[test]
    fn badge_sampler_uses_curve_length_and_clears_prior_badges() {
        let sampler = PathSampler::new("M0,0L100,0", &[]);
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let first = choose_position(&sampler, &[], &work).unwrap().unwrap();
        let second = choose_position(&sampler, &[Obstacle::square(first, 9.0)], &work)
            .unwrap()
            .unwrap();
        assert_eq!(first, kurbo::Point::new(25.0, 0.0));
        assert!(first.distance(second) >= 20.0);
        let singleton = PathSampler::new("", &[LayoutPoint { x: 1.0, y: 2.0 }]);
        assert_eq!(singleton.sample(0.25), Some(kurbo::Point::new(1.0, 2.0)));
    }

    #[test]
    fn badge_samples_the_emitted_path_but_charges_the_logical_path() {
        let config = PresentationConfig::empty_object();
        let work = OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
        let placement = StepBadgePlacement::new(&config, &work);
        let logical = "M1,1L20,30L90,10";
        let point = placement
            .place(logical, Some("M0,0L100,0"), &[], Vec2::new(10.0, 20.0))
            .unwrap();

        assert_eq!(point, Some(Point::new(35.0, 20.0)));
        // One segment's sampling allowance and the first unobstructed candidate.
        assert_eq!(work.used(), logical.len() + 63 + 1);
    }
}
