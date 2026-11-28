use super::{
    CLEARANCE, GridPortalRange, fixed_point, invalid, normalize_point, normalize_rect, reserved,
    side_coordinate,
};
use crate::graph_layout::{Axis, Side};
use crate::resources::OperationWorkMeter;
use kurbo::{Point, Rect};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridPairedPortal<'a> {
    pub owner_id: &'a str,
    pub side: Side,
    pub tangential_coordinate: f64,
    pub interior: Point,
    pub exterior: Point,
    pub axis: Axis,
    pub length: f64,
}

/// The caller supplies the measured shell and optional title. Apply the interior/exterior
/// normal offsets to each range when it is assigned to a container topology.
pub fn grid_portal_ranges<'a>(
    owner_id: &'a str,
    bounds: Rect,
    title: Option<Rect>,
    work: &OperationWorkMeter,
) -> crate::Result<Vec<GridPortalRange<'a>>> {
    let mut ranges = reserved(4, work)?;
    let bounds = normalize_rect(bounds)?;
    let title = title.map(normalize_rect).transpose()?;
    let title_bottom = title.map_or(f64::NEG_INFINITY, |rect| rect.y1);
    let vertical_low = (bounds.y0 + CLEARANCE).max(title_bottom + CLEARANCE);
    let vertical_high = bounds.y1 - CLEARANCE;
    let horizontal_low = bounds.x0 + CLEARANCE;
    let horizontal_high = bounds.x1 - CLEARANCE;
    if vertical_low <= vertical_high {
        for side in [Side::Left, Side::Right] {
            ranges.push(GridPortalRange {
                owner_id,
                side,
                low: vertical_low,
                high: vertical_high,
                coordinate: None,
            });
        }
    }
    if horizontal_low <= horizontal_high {
        let bottom = GridPortalRange {
            owner_id,
            side: Side::Bottom,
            low: horizontal_low,
            high: horizontal_high,
            coordinate: None,
        };
        ranges.push(bottom);
        if title_bottom + CLEARANCE <= bounds.y0 {
            ranges.push(GridPortalRange {
                side: Side::Top,
                ..bottom
            });
        }
    }
    Ok(ranges)
}

pub fn grid_paired_portal<'a>(
    owner_id: &'a str,
    bounds: Rect,
    title: Option<Rect>,
    side: Side,
    tangential_coordinate: f64,
    work: &OperationWorkMeter,
) -> crate::Result<GridPairedPortal<'a>> {
    let bounds = normalize_rect(bounds)?;
    let legal = grid_portal_ranges(owner_id, bounds, title, work)?
        .iter()
        .any(|range| {
            range.side == side
                && range.low <= tangential_coordinate
                && tangential_coordinate <= range.high
        });
    if !legal {
        return Err(invalid("Illegal Grid portal coordinate"));
    }
    let boundary = side_coordinate(bounds, side);
    let toward_inside = if matches!(side, Side::Left | Side::Top) {
        CLEARANCE
    } else {
        -CLEARANCE
    };
    let axis = side.axis();
    Ok(GridPairedPortal {
        owner_id,
        side,
        tangential_coordinate,
        axis,
        length: 2.0 * CLEARANCE,
        interior: normalize_point(fixed_point(
            axis,
            boundary + toward_inside,
            tangential_coordinate,
        ))?,
        exterior: normalize_point(fixed_point(
            axis,
            boundary - toward_inside,
            tangential_coordinate,
        ))?,
    })
}
