//! Modified EPL-2.0 translation of AreaApproximation, Calculations, and the three candidate filters.

use super::{Error, Options, Placement, Rectangle, charge};
use crate::work::WorkControl;

#[derive(Clone, Copy)]
struct Candidate {
    kind: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

pub(super) fn target_width(
    rectangles: &[Rectangle],
    options: &Options,
    work: &mut dyn WorkControl,
) -> Result<f64, Error> {
    charge(work, rectangles.len())?;
    let mut placed = Vec::with_capacity(rectangles.len());
    let first = rectangles[0];
    placed.push(Placement {
        width: first.width,
        height: first.height,
        ..Default::default()
    });
    let mut width = first.width;
    let mut height = first.height;
    let spacing = options.spacing;
    for rectangle in &rectangles[1..] {
        charge(work, placed.len())?;
        let last = placed[placed.len() - 1];
        let right_x = last.x + last.width + spacing;
        let below_y = last.y + last.height + spacing;
        let mut right_y: Option<f64> = None;
        let mut below_x: Option<f64> = None;
        for previous in &placed {
            if right_x < previous.x + previous.width + spacing {
                right_y = Some(right_y.map_or(previous.y + previous.height, |old| {
                    old.max(previous.y + previous.height)
                }));
            }
            if below_y < previous.y + previous.height + spacing {
                below_x = Some(below_x.map_or(previous.x + previous.width, |old| {
                    old.max(previous.x + previous.width)
                }));
            }
        }
        let coordinates = [
            (right_x, right_y.map_or(0.0, |y| y + spacing)),
            (below_x.map_or(0.0, |x| x + spacing), below_y),
            (width + spacing, 0.0),
            (0.0, height + spacing),
        ];
        let mut candidates: Vec<_> = coordinates
            .into_iter()
            .enumerate()
            .map(|(kind, (x, y))| Candidate {
                kind,
                x,
                y,
                width: width.max(x + rectangle.width),
                height: height.max(y + rectangle.height),
            })
            .collect();
        for criterion in 0..3 {
            if candidates.len() <= 1 {
                break;
            }
            let score = |candidate: &Candidate| {
                let w = candidate.width + options.padding.left + options.padding.right;
                let h = candidate.height + options.padding.top + options.padding.bottom;
                match criterion {
                    0 => -(options.aspect_ratio / w).min(1.0 / h),
                    1 => w * h,
                    _ => (w / h - options.aspect_ratio).abs(),
                }
            };
            let best = candidates.iter().map(score).fold(f64::INFINITY, f64::min);
            candidates.retain(|candidate| score(candidate) == best);
        }
        let chosen = match candidates.as_slice() {
            [only] => *only,
            [a, b] => {
                if a.kind % 2 == b.kind % 2 {
                    *b
                } else if a.kind == 0 && b.kind == 1 {
                    let area_right = (a.x + rectangle.width - last.x)
                        * ((last.y + last.height).max(a.y + rectangle.height) - last.y.min(a.y));
                    let area_below = ((last.x + last.width).max(b.x + rectangle.width)
                        - last.x.min(b.x))
                        * (b.y + rectangle.height - last.y);
                    if area_right <= area_below { *a } else { *b }
                } else {
                    *a
                }
            }
            // Source returns null for an ambiguous/nonfinite candidate set. Report a deterministic
            // error instead of propagating that into unchecked indexing or invalid coordinates.
            _ => return Err(Error::NonFiniteGeometry),
        };
        if !chosen.width.is_finite() || !chosen.height.is_finite() {
            return Err(Error::NonFiniteGeometry);
        }
        width = chosen.width;
        height = chosen.height;
        placed.push(Placement {
            x: chosen.x,
            y: chosen.y,
            width: rectangle.width,
            height: rectangle.height,
            ..Default::default()
        });
    }
    Ok(width)
}
