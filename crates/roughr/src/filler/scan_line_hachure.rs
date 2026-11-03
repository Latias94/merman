use std::borrow::BorrowMut;
use std::cmp::Ordering;
use std::marker::PhantomData;

use euclid::default::Point2D;
use euclid::Trig;
use num_traits::{Float, FromPrimitive};

use super::traits::PatternFiller;
use crate::core::{_c, OpSet, Options};
use crate::geometry::{rotate_lines_with_control, rotate_points_with_control, Line};
use crate::work::{advance, finite, sort_by, UnlimitedWork, WorkControl, WorkError};

#[derive(Clone)]
struct EdgeEntry<F: Float + FromPrimitive + Trig> {
    pub(crate) ymin: F,
    pub(crate) ymax: F,
    pub(crate) x: F,
    pub(crate) islope: F,
}

impl<F: Float + FromPrimitive + Trig> std::fmt::Display for EdgeEntry<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ymin={} ymax={} x={} islope={}",
            self.ymin.to_f64().unwrap(),
            self.ymax.to_f64().unwrap(),
            self.x.to_f64().unwrap(),
            self.islope.to_f64().unwrap()
        )
    }
}

#[derive(Clone)]
struct ActiveEdgeEntry<F: Float + FromPrimitive + Trig> {
    pub(crate) edge: EdgeEntry<F>,
}

pub fn polygon_hachure_lines<F: Float + FromPrimitive + Trig>(
    polygon_list: &mut [Vec<Point2D<F>>],
    options: &Options,
) -> Vec<Line<F>> {
    polygon_hachure_lines_with_control(polygon_list, options, &mut UnlimitedWork)
        .expect("invalid hachure geometry")
}

pub fn polygon_hachure_lines_with_control<F: Float + FromPrimitive + Trig>(
    polygon_list: &mut [Vec<Point2D<F>>],
    options: &Options,
    control: &mut impl WorkControl,
) -> Result<Vec<Line<F>>, WorkError> {
    control.charge(1)?;
    crate::renderer::validate_options(options)?;
    let angle = options.hachure_angle.unwrap_or(0.0) + 90.0;
    let mut gap = options.hachure_gap.unwrap_or(0.0);
    if gap < 0.0 {
        gap = options.stroke_width.unwrap_or(0.0) * 4.0;
    }

    gap = f32::max(gap, 0.1);
    finite(&[angle, gap])?;

    let center = Point2D::new(_c(0.0), _c(0.0));
    if angle != 0.0 {
        rotate_polygons(polygon_list, &center, _c(angle), control)?;
    }

    let mut lines = straight_hachure_lines_with_control(polygon_list, _c(gap), control)?;

    if angle != 0.0 {
        rotate_polygons(polygon_list, &center, _c(-angle), control)?;
        lines = rotate_lines_with_control(&lines, &center, _c(-angle), control)?;
    }

    Ok(lines)
}

fn rotate_polygons<F: Float + FromPrimitive + Trig>(
    polygons: &mut [Vec<Point2D<F>>],
    center: &Point2D<F>,
    angle: F,
    control: &mut impl WorkControl,
) -> Result<(), WorkError> {
    for polygon in polygons {
        control.charge(1)?;
        *polygon = rotate_points_with_control(polygon, center, angle, control)?;
    }
    Ok(())
}

#[cfg(test)]
fn straight_hachure_lines<F>(polygon_list: &mut [Vec<Point2D<F>>], gap: F) -> Vec<Line<F>>
where
    F: Float + FromPrimitive + Trig,
{
    straight_hachure_lines_with_control(polygon_list, gap, &mut UnlimitedWork)
        .expect("invalid scanline geometry")
}

fn straight_hachure_lines_with_control<F: Float + FromPrimitive + Trig>(
    polygon_list: &mut [Vec<Point2D<F>>],
    gap: F,
    control: &mut impl WorkControl,
) -> Result<Vec<Line<F>>, WorkError> {
    finite(&[gap])?;
    for polygon in polygon_list.iter_mut() {
        control.charge(1)?;
        if polygon.first() != polygon.last() {
            control.charge(1)?;
            polygon.push(
                *polygon
                    .first()
                    .expect("can not get first element of polygon"),
            );
        }
    }

    let mut lines: Vec<Line<F>> = vec![];
    let gap = F::max(gap, _c(0.1));

    // create sorted edges table
    let mut edges: Vec<EdgeEntry<F>> = vec![];

    for vertices in polygon_list.iter().filter(|vertices| vertices.len() > 2) {
        control.charge(1)?;
        for w in vertices.windows(2) {
            control.charge(1)?;
            let p1 = w[0];
            let p2 = w[1];
            finite(&[p1.x, p1.y, p2.x, p2.y])?;
            if p1.y != p2.y {
                let ymin = F::min(p1.y, p2.y);
                let islope = (p2.x - p1.x) / (p2.y - p1.y);
                finite(&[islope])?;
                edges.push(EdgeEntry {
                    ymin,
                    ymax: F::max(p1.y, p2.y),
                    x: if ymin == p1.y { p1.x } else { p2.x },
                    islope,
                });
            }
        }
    }

    sort_by(
        &mut edges,
        |e1, e2| {
            if e1.ymin < e2.ymin {
                Ordering::Less
            } else if e1.ymin > e2.ymin {
                Ordering::Greater
            } else if e1.x < e2.x {
                Ordering::Less
            } else if e1.x > e2.x {
                Ordering::Greater
            } else if e1.ymax == e2.ymax {
                Ordering::Equal
            } else {
                if e1.ymax > e2.ymax {
                    Ordering::Greater
                } else if e1.ymax < e2.ymax {
                    Ordering::Less
                } else {
                    Ordering::Equal
                }
            }
        },
        control,
    )?;

    if edges.is_empty() {
        return Ok(lines);
    }

    let mut active_edges: Vec<ActiveEdgeEntry<F>> = Vec::new();
    let mut y = edges.first().unwrap().ymin;
    let mut next_edge = 0;

    loop {
        control.charge(1)?;
        while next_edge < edges.len() {
            control.charge(1)?;
            if edges[next_edge].ymin > y {
                break;
            }
            active_edges.push(ActiveEdgeEntry {
                edge: edges[next_edge].clone(),
            });
            next_edge += 1;
        }

        let mut retained = 0;
        for index in 0..active_edges.len() {
            control.charge(1)?;
            if active_edges[index].edge.ymax > y {
                active_edges.swap(retained, index);
                retained += 1;
            }
        }
        active_edges.truncate(retained);

        sort_by(
            &mut active_edges,
            |ae1, ae2| {
                if ae1.edge.x == ae2.edge.x {
                    Ordering::Equal
                } else {
                    if ae1.edge.x > ae2.edge.x {
                        Ordering::Greater
                    } else {
                        Ordering::Less
                    }
                }
            },
            control,
        )?;
        if active_edges.len() > 1 {
            if !active_edges.len().is_multiple_of(2) {
                return Err(WorkError::InvalidGeometry);
            }
            for ae in active_edges.chunks_exact(2) {
                control.charge(1)?;
                let ce = &ae[0];
                let ne = &ae[1];
                lines.push(Line::from(&[
                    euclid::Point2D::new(ce.edge.x, y),
                    euclid::Point2D::new(ne.edge.x, y),
                ]));
            }
        }

        y = advance(y, gap)?;
        for ae in &mut active_edges {
            control.charge(1)?;
            ae.edge.x = ae.edge.x + (gap * ae.edge.islope);
            finite(&[ae.edge.x])?;
        }
        if next_edge == edges.len() && active_edges.is_empty() {
            break;
        }
    }

    Ok(lines)
}

pub struct ScanlineHachureFiller<F> {
    _phantom: PhantomData<F>,
}

impl<F, P> PatternFiller<F, P> for ScanlineHachureFiller<F>
where
    F: Float + Trig + FromPrimitive,
    P: BorrowMut<Vec<Vec<Point2D<F>>>>,
{
    fn fill_polygons(&self, mut polygon_list: P, o: &mut Options) -> crate::core::OpSet<F> {
        self.fill_polygons_with_control(polygon_list.borrow_mut(), o, &mut UnlimitedWork)
            .expect("invalid hachure fill")
    }
}

impl<F: Float + Trig + FromPrimitive> ScanlineHachureFiller<F> {
    pub fn fill_polygons_with_control(
        &self,
        polygons: &mut [Vec<Point2D<F>>],
        options: &mut Options,
        control: &mut impl WorkControl,
    ) -> Result<OpSet<F>, WorkError> {
        let lines = polygon_hachure_lines_with_control(polygons, options, control)?;
        let mut ops = Vec::new();
        for line in lines {
            control.charge(4)?;
            finite(&[(line.start_point.x - line.end_point.x).powi(2)
                + (line.start_point.y - line.end_point.y).powi(2)])?;
            let mut line_ops = crate::renderer::_double_line(
                line.start_point.x,
                line.start_point.y,
                line.end_point.x,
                line.end_point.y,
                options,
                true,
            );
            for operation in &line_ops {
                finite(&operation.data)?;
            }
            ops.append(&mut line_ops);
        }
        Ok(OpSet {
            op_set_type: crate::core::OpSetType::FillSketch,
            ops,
            size: None,
            path: None,
        })
    }

    pub fn new() -> Self {
        ScanlineHachureFiller {
            _phantom: PhantomData,
        }
    }
}

impl<F: Float + Trig + FromPrimitive> Default for ScanlineHachureFiller<F> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use euclid::point2;

    use crate::geometry::Line;

    #[test]
    fn straight_hachure_lines() {
        let mut input = vec![vec![
            point2(0.0, 0.0),
            point2(0.0, 1.0),
            point2(1.0, 1.0),
            point2(1.0, 0.0),
        ]];
        let expected = vec![
            Line::from(&[point2(0.0, 0.0), point2(1.0, 0.0)]),
            Line::from(&[
                point2(0.0, 0.10000000149011612),
                point2(1.0, 0.10000000149011612),
            ]),
            Line::from(&[
                point2(0.0, 0.20000000298023224),
                point2(1.0, 0.20000000298023224),
            ]),
            Line::from(&[
                point2(0.0, 0.30000000447034836),
                point2(1.0, 0.30000000447034836),
            ]),
            Line::from(&[
                point2(0.0, 0.4000000059604645),
                point2(1.0, 0.4000000059604645),
            ]),
            Line::from(&[
                point2(0.0, 0.5000000074505806),
                point2(1.0, 0.5000000074505806),
            ]),
            Line::from(&[
                point2(0.0, 0.6000000089406967),
                point2(1.0, 0.6000000089406967),
            ]),
            Line::from(&[
                point2(0.0, 0.7000000104308128),
                point2(1.0, 0.7000000104308128),
            ]),
            Line::from(&[
                point2(0.0, 0.800000011920929),
                point2(1.0, 0.800000011920929),
            ]),
            Line::from(&[
                point2(0.0, 0.9000000134110451),
                point2(1.0, 0.9000000134110451),
            ]),
        ];
        let result = super::straight_hachure_lines(&mut input, 0.1);
        assert_eq!(expected, result);
    }
}
