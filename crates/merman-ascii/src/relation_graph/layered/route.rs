use super::super::RelationGraphLabel;
use super::boxes::PlacedRelationGraphBox;
use super::draw::{
    RelationLineChars, draw_relation_span_exclusive, draw_relation_span_inclusive,
    put_relation_char, write_centered_relation_label, write_centered_relation_text,
};
use crate::AsciiError;
use crate::Result;
use crate::canvas::Canvas;
use crate::color::AsciiColorRole;
use crate::options::TerminalWidthProfile;
use crate::resource::ResourceContext;
use crate::text::display_width_with_profile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RelationOverlay {
    Glyph {
        x: usize,
        y: usize,
        ch: char,
        role: AsciiColorRole,
    },
    Text {
        center_x: usize,
        y: usize,
        text: String,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
    },
    Label {
        center_x: usize,
        y: usize,
        label: RelationGraphLabel,
        role: AsciiColorRole,
    },
}

impl RelationOverlay {
    pub(crate) fn glyph(x: usize, y: usize, ch: char, role: AsciiColorRole) -> Self {
        Self::Glyph { x, y, ch, role }
    }

    pub(crate) fn text(
        center_x: usize,
        y: usize,
        text: String,
        role: AsciiColorRole,
        width_profile: TerminalWidthProfile,
    ) -> Self {
        Self::Text {
            center_x,
            y,
            text,
            role,
            width_profile,
        }
    }

    pub(crate) fn label(
        center_x: usize,
        y: usize,
        label: RelationGraphLabel,
        role: AsciiColorRole,
    ) -> Self {
        Self::Label {
            center_x,
            y,
            label,
            role,
        }
    }

    fn draw_at(&self, canvas: &mut Canvas) -> Result<()> {
        match self {
            RelationOverlay::Glyph { x, y, ch, role } => canvas.try_set_role(*x, *y, *ch, *role),
            RelationOverlay::Text {
                center_x,
                y,
                text,
                role,
                width_profile,
            } => write_centered_relation_text(canvas, *center_x, *y, text, *role, *width_profile),
            RelationOverlay::Label {
                center_x,
                y,
                label,
                role,
            } => write_centered_relation_label(canvas, *center_x, *y, label, *role),
        }
    }

    fn fits(&self, width: usize, height: usize) -> bool {
        match self {
            Self::Glyph { x, y, .. } => *x < width && *y < height,
            Self::Text {
                center_x,
                y,
                text,
                width_profile,
                ..
            } => centered_text_fits(
                *center_x,
                *y,
                display_width_with_profile(text, *width_profile),
                1,
                width,
                height,
            ),
            Self::Label {
                center_x, y, label, ..
            } => centered_text_fits(
                *center_x,
                *y,
                label.width(),
                label.line_count(),
                width,
                height,
            ),
        }
    }

    fn bounds(&self) -> Option<RelationOverlayBounds> {
        let (center_x, y, width, height) = match self {
            Self::Glyph { x, y, .. } => return RelationOverlayBounds::new(*x, *y, 1, 1),
            Self::Text {
                center_x,
                y,
                text,
                width_profile,
                ..
            } => (
                *center_x,
                *y,
                display_width_with_profile(text, *width_profile),
                1,
            ),
            Self::Label {
                center_x, y, label, ..
            } => (*center_x, *y, label.width(), label.line_count()),
        };
        let left = center_x.checked_sub(width / 2)?;
        RelationOverlayBounds::new(left, y, width, height)
    }

    pub(crate) fn overlaps_rect(
        &self,
        left: usize,
        top: usize,
        right: usize,
        bottom: usize,
    ) -> bool {
        self.bounds().is_some_and(|bounds| {
            bounds.left <= right
                && left < bounds.right
                && bounds.top <= bottom
                && top < bounds.bottom
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RelationOverlayBounds {
    left: usize,
    right: usize,
    top: usize,
    bottom: usize,
}

impl RelationOverlayBounds {
    fn new(left: usize, top: usize, width: usize, height: usize) -> Option<Self> {
        let right = left.checked_add(width)?;
        let bottom = top.checked_add(height)?;
        Some(Self {
            left,
            right,
            top,
            bottom,
        })
    }

    fn overlaps(self, other: Self) -> bool {
        self.left < other.right
            && other.left < self.right
            && self.top < other.bottom
            && other.top < self.bottom
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayeredRelationRouteGeometry {
    from_x: usize,
    to_x: usize,
    source_path_start_y: usize,
    source_marker_y: usize,
    route_y: usize,
    target_marker_y: usize,
    target_path_end_y: usize,
}

impl LayeredRelationRouteGeometry {
    pub(crate) fn source_x(&self) -> usize {
        self.from_x
    }

    pub(crate) fn target_x(&self) -> usize {
        self.to_x
    }

    pub(crate) fn route_y(&self) -> usize {
        self.route_y
    }

    pub(crate) fn source_marker_y(&self) -> usize {
        self.source_marker_y
    }

    pub(crate) fn target_marker_y(&self) -> usize {
        self.target_marker_y
    }

    pub(crate) fn label_y_after_source(&self) -> usize {
        if self.source_marker_y <= self.target_marker_y {
            return self
                .source_marker_y
                .checked_add(1)
                .unwrap_or(self.route_y())
                .min(self.route_y());
        }

        self.source_marker_y
            .checked_sub(1)
            .unwrap_or(self.route_y())
            .max(self.route_y())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayeredRelationRoutePlan {
    geometry: LayeredRelationRouteGeometry,
    vertical_char: char,
    horizontal_char: char,
    relation_chars: RelationLineChars,
    overlays: Vec<RelationOverlay>,
}

impl LayeredRelationRoutePlan {
    pub(crate) fn new(
        geometry: LayeredRelationRouteGeometry,
        vertical_char: char,
        horizontal_char: char,
        relation_chars: RelationLineChars,
        overlays: Vec<RelationOverlay>,
    ) -> Self {
        Self {
            geometry,
            vertical_char,
            horizontal_char,
            relation_chars,
            overlays,
        }
    }

    pub(crate) fn draw_route_at(&self, canvas: &mut Canvas) -> Result<()> {
        draw_relation_span_inclusive(
            canvas,
            self.geometry.from_x,
            self.geometry.source_path_start_y,
            self.geometry.route_y,
            self.vertical_char,
            self.relation_chars,
        )?;
        if self.geometry.from_x != self.geometry.to_x {
            let left = self.geometry.from_x.min(self.geometry.to_x);
            let right = self.geometry.from_x.max(self.geometry.to_x);
            for x in left..=right {
                put_relation_char(
                    canvas,
                    x,
                    self.geometry.route_y,
                    self.horizontal_char,
                    self.relation_chars,
                )?;
            }
        }
        draw_relation_span_exclusive(
            canvas,
            self.geometry.to_x,
            self.geometry.route_y,
            self.geometry.target_path_end_y,
            self.vertical_char,
            self.relation_chars,
        )?;
        Ok(())
    }

    pub(crate) fn source_x(&self) -> usize {
        self.geometry.source_x()
    }

    pub(crate) fn target_x(&self) -> usize {
        self.geometry.target_x()
    }

    pub(crate) fn draw_overlays_at(&self, canvas: &mut Canvas) -> Result<()> {
        for overlay in &self.overlays {
            overlay.draw_at(canvas)?;
        }
        Ok(())
    }

    pub(crate) fn route_fits(&self, width: usize, height: usize) -> bool {
        [
            (self.geometry.from_x, self.geometry.source_path_start_y),
            (self.geometry.from_x, self.geometry.route_y),
            (self.geometry.to_x, self.geometry.route_y),
            (self.geometry.to_x, self.geometry.target_path_end_y),
        ]
        .into_iter()
        .all(|(x, y)| x < width && y < height)
    }

    pub(crate) fn overlays_fit(&self, width: usize, height: usize) -> bool {
        self.overlays
            .iter()
            .all(|overlay| overlay.fits(width, height))
    }

    pub(crate) fn overlays_overlap(&self, other: &Self) -> bool {
        self.overlays.iter().any(|left| {
            other.overlays.iter().any(|right| {
                if left == right {
                    return false;
                }
                left.bounds()
                    .zip(right.bounds())
                    .is_some_and(|(left, right)| left.overlaps(right))
            })
        })
    }

    pub(crate) fn route_overlaps_rect(
        &self,
        left: usize,
        top: usize,
        right: usize,
        bottom: usize,
    ) -> bool {
        let geometry = &self.geometry;
        vertical_inclusive_overlaps_rect(
            geometry.from_x,
            geometry.source_path_start_y,
            geometry.route_y,
            left,
            top,
            right,
            bottom,
        ) || horizontal_inclusive_overlaps_rect(
            geometry.from_x,
            geometry.to_x,
            geometry.route_y,
            left,
            top,
            right,
            bottom,
        ) || vertical_exclusive_overlaps_rect(
            geometry.to_x,
            geometry.route_y,
            geometry.target_path_end_y,
            left,
            top,
            right,
            bottom,
        )
    }

    pub(crate) fn overlays_overlap_rect(
        &self,
        left: usize,
        top: usize,
        right: usize,
        bottom: usize,
    ) -> bool {
        self.overlays
            .iter()
            .any(|overlay| overlay.overlaps_rect(left, top, right, bottom))
    }
}

#[allow(clippy::too_many_arguments)]
fn vertical_inclusive_overlaps_rect(
    x: usize,
    start_y: usize,
    end_y: usize,
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
) -> bool {
    let segment_top = start_y.min(end_y);
    let segment_bottom = start_y.max(end_y);
    (left..=right).contains(&x) && segment_top <= bottom && top <= segment_bottom
}

#[allow(clippy::too_many_arguments)]
fn vertical_exclusive_overlaps_rect(
    x: usize,
    start_y: usize,
    end_y: usize,
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
) -> bool {
    if !(left..=right).contains(&x) || start_y == end_y {
        return false;
    }
    let (segment_top, segment_bottom) = if start_y < end_y {
        (start_y, end_y - 1)
    } else {
        let Some(segment_top) = end_y.checked_add(1) else {
            return false;
        };
        (segment_top, start_y)
    };
    segment_top <= bottom && top <= segment_bottom
}

#[allow(clippy::too_many_arguments)]
fn horizontal_inclusive_overlaps_rect(
    start_x: usize,
    end_x: usize,
    y: usize,
    left: usize,
    top: usize,
    right: usize,
    bottom: usize,
) -> bool {
    let segment_left = start_x.min(end_x);
    let segment_right = start_x.max(end_x);
    (top..=bottom).contains(&y) && segment_left <= right && left <= segment_right
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayeredRelationRouteProfile {
    min_vertical_gap: usize,
    source_path_start_offset: usize,
    route_y_offset_from_target: usize,
    target_path_end_offset_from_target: usize,
    endpoint_label_gap: usize,
}

impl LayeredRelationRouteProfile {
    pub(crate) const fn new(
        min_vertical_gap: usize,
        source_path_start_offset: usize,
        route_y_offset_from_target: usize,
        target_path_end_offset_from_target: usize,
        endpoint_label_gap: usize,
    ) -> Self {
        Self {
            min_vertical_gap,
            source_path_start_offset,
            route_y_offset_from_target,
            target_path_end_offset_from_target,
            endpoint_label_gap,
        }
    }

    pub(crate) const fn class() -> Self {
        Self::new(1, 1, 1, 0, 0)
    }

    pub(crate) const fn class_with_endpoint_labels(endpoint_label_gap: usize) -> Self {
        Self::new(1, 1, 1, 0, endpoint_label_gap)
    }

    pub(crate) const fn er() -> Self {
        Self::new(2, 2, 2, 1, 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayeredRelationRouteStyle {
    vertical_char: char,
    horizontal_char: char,
    relation_chars: RelationLineChars,
    profile: LayeredRelationRouteProfile,
}

impl LayeredRelationRouteStyle {
    pub(crate) const fn new(
        vertical_char: char,
        horizontal_char: char,
        relation_chars: RelationLineChars,
        profile: LayeredRelationRouteProfile,
    ) -> Self {
        Self {
            vertical_char,
            horizontal_char,
            relation_chars,
            profile,
        }
    }

    pub(crate) const fn profile(self) -> LayeredRelationRouteProfile {
        self.profile
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LayeredRelationRouteRequest<'boxes, 'graph> {
    placed_boxes: &'boxes [PlacedRelationGraphBox<'graph>],
    top: &'boxes PlacedRelationGraphBox<'graph>,
    bottom: &'boxes PlacedRelationGraphBox<'graph>,
    lane_offset: isize,
    profile: LayeredRelationRouteProfile,
}

impl<'boxes, 'graph> LayeredRelationRouteRequest<'boxes, 'graph> {
    pub(crate) fn new(
        placed_boxes: &'boxes [PlacedRelationGraphBox<'graph>],
        top: &'boxes PlacedRelationGraphBox<'graph>,
        bottom: &'boxes PlacedRelationGraphBox<'graph>,
        lane_offset: isize,
        profile: LayeredRelationRouteProfile,
    ) -> Self {
        Self {
            placed_boxes,
            top,
            bottom,
            lane_offset,
            profile,
        }
    }
}

pub(crate) fn plan_layered_relation_route_draw(
    request: LayeredRelationRouteRequest<'_, '_>,
    style: LayeredRelationRouteStyle,
    resources: &mut ResourceContext,
    build_overlays: impl FnOnce(
        &LayeredRelationRouteGeometry,
        &mut ResourceContext,
    ) -> Result<Vec<RelationOverlay>>,
) -> Result<LayeredRelationRoutePlan> {
    resources.charge_layout_work(request.placed_boxes.len().max(1))?;
    let geometry = plan_layered_relation_route(request, resources)?;
    let overlays = build_overlays(&geometry, resources)?;
    resources.charge_layout_work(overlays.len().max(1))?;
    let vertical_work = geometry
        .source_path_start_y
        .abs_diff(geometry.route_y)
        .checked_add(geometry.route_y.abs_diff(geometry.target_path_end_y))
        .ok_or_else(|| work_overflow(resources))?;
    let route_work = vertical_work
        .checked_add(geometry.from_x.abs_diff(geometry.to_x))
        .and_then(|value| value.checked_add(3))
        .ok_or_else(|| work_overflow(resources))?;
    resources.charge_layout_work(route_work)?;
    Ok(LayeredRelationRoutePlan::new(
        geometry,
        style.vertical_char,
        style.horizontal_char,
        style.relation_chars,
        overlays,
    ))
}

pub(crate) fn offset_center(
    center: usize,
    offset: isize,
    resources: &ResourceContext,
) -> Result<usize> {
    if offset < 0 {
        center
            .checked_sub(offset.unsigned_abs())
            .ok_or_else(|| grid_overflow(resources))
    } else {
        center
            .checked_add(usize::try_from(offset).map_err(|_| grid_overflow(resources))?)
            .ok_or_else(|| grid_overflow(resources))
    }
}

pub(crate) fn spanning_lane_offset(
    top_width: usize,
    bottom_width: usize,
    resources: &ResourceContext,
) -> Result<isize> {
    let offset = resources.checked_grid_add(top_width.max(bottom_width) / 2, 3)?;
    isize::try_from(offset).map_err(|_| grid_overflow(resources))
}

pub(crate) fn spanning_lane_offset_around_intermediate_boxes(
    placed_boxes: &[PlacedRelationGraphBox<'_>],
    top: &PlacedRelationGraphBox<'_>,
    bottom: &PlacedRelationGraphBox<'_>,
    lane_offset: isize,
    resources: &mut ResourceContext,
) -> Result<isize> {
    resources.charge_layout_work(placed_boxes.len().max(1))?;
    let lower_bound = top.y().min(bottom.y());
    let upper_bound = top.bottom().max(bottom.bottom());
    let mut intermediate_boxes: Vec<&PlacedRelationGraphBox<'_>> = Vec::new();
    intermediate_boxes
        .try_reserve_exact(placed_boxes.len())
        .map_err(|_| layout_allocation_failed())?;
    intermediate_boxes.extend(
        placed_boxes
            .iter()
            .filter(|placed_box| placed_box.y() > lower_bound && placed_box.bottom() < upper_bound),
    );
    if intermediate_boxes.is_empty() {
        return Ok(lane_offset);
    }

    let route_clearance = intermediate_boxes
        .iter()
        .map(|placed_box| placed_box.width() / 2)
        .max()
        .unwrap_or(0);
    let spanning_offset = spanning_lane_offset(
        top.width()
            .max(resources.checked_grid_mul(route_clearance, 2)?),
        bottom.width(),
        resources,
    )?;
    let left_offset = lane_offset
        .checked_sub(spanning_offset)
        .ok_or_else(|| grid_overflow(resources))?;
    let right_offset = lane_offset
        .checked_add(spanning_offset)
        .ok_or_else(|| grid_overflow(resources))?;
    let left_is_clear = lane_offset_fits_center(bottom.center_x(), left_offset)
        && !route_column_crosses_any_box(
            top.center_x(),
            left_offset,
            &intermediate_boxes,
            resources,
        )?;
    let right_is_clear = lane_offset_fits_center(bottom.center_x(), right_offset)
        && !route_column_crosses_any_box(
            top.center_x(),
            right_offset,
            &intermediate_boxes,
            resources,
        )?;

    Ok(match (left_is_clear, right_is_clear) {
        (true, false) => left_offset,
        (false, true) => right_offset,
        (true, true) if lane_offset < 0 => left_offset,
        (true, true) if top_is_left_of_intermediate_boxes(top, &intermediate_boxes) => left_offset,
        (true, true) => right_offset,
        (false, false) if top_is_left_of_intermediate_boxes(top, &intermediate_boxes) => {
            route_column_left_of_intermediate_boxes(top, &intermediate_boxes, resources)?
        }
        (false, false) => {
            route_column_right_of_intermediate_boxes(top, &intermediate_boxes, resources)?
        }
    })
}

pub(crate) fn plan_layered_relation_route(
    request: LayeredRelationRouteRequest<'_, '_>,
    resources: &mut ResourceContext,
) -> Result<LayeredRelationRouteGeometry> {
    let lane_offset = spanning_lane_offset_around_intermediate_boxes(
        request.placed_boxes,
        request.top,
        request.bottom,
        request.lane_offset,
        resources,
    )?;
    let from_x = offset_center(request.top.center_x(), lane_offset, resources)?;
    let to_x = offset_center(request.bottom.center_x(), lane_offset, resources)?;
    let source_top = request.top.y();
    let source_bottom = request.top.bottom();
    let target_top = request.bottom.y();
    let target_bottom = request.bottom.bottom();
    let endpoint_label_gap = request.profile.endpoint_label_gap;

    if target_top > resources.checked_grid_add(source_bottom, request.profile.min_vertical_gap)? {
        return Ok(LayeredRelationRouteGeometry {
            from_x,
            to_x,
            source_path_start_y: checked_add_gap(
                source_bottom,
                request.profile.source_path_start_offset,
                endpoint_label_gap,
                resources,
            )?,
            source_marker_y: checked_add_gap(source_bottom, 1, endpoint_label_gap, resources)?,
            route_y: checked_sub_gap(
                target_top,
                request.profile.route_y_offset_from_target,
                endpoint_label_gap,
                resources,
            )?,
            target_marker_y: checked_sub_gap(target_top, 1, endpoint_label_gap, resources)?,
            target_path_end_y: checked_sub_gap(
                target_top,
                request.profile.target_path_end_offset_from_target,
                endpoint_label_gap,
                resources,
            )?,
        });
    }

    if source_top > resources.checked_grid_add(target_bottom, request.profile.min_vertical_gap)? {
        return Ok(LayeredRelationRouteGeometry {
            from_x,
            to_x,
            source_path_start_y: checked_sub_gap(
                source_top,
                request.profile.source_path_start_offset,
                endpoint_label_gap,
                resources,
            )?,
            source_marker_y: checked_sub_gap(source_top, 1, endpoint_label_gap, resources)?,
            route_y: checked_add_gap(
                target_bottom,
                request.profile.route_y_offset_from_target,
                endpoint_label_gap,
                resources,
            )?,
            target_marker_y: checked_add_gap(target_bottom, 1, endpoint_label_gap, resources)?,
            target_path_end_y: checked_add_gap(
                target_bottom,
                request.profile.target_path_end_offset_from_target,
                endpoint_label_gap,
                resources,
            )?,
        });
    }

    Ok(LayeredRelationRouteGeometry {
        from_x,
        to_x,
        source_path_start_y: checked_add_gap(
            source_bottom,
            request.profile.source_path_start_offset,
            endpoint_label_gap,
            resources,
        )?,
        source_marker_y: checked_add_gap(source_bottom, 1, endpoint_label_gap, resources)?,
        route_y: checked_add_gap(
            source_bottom.max(target_bottom),
            request.profile.route_y_offset_from_target,
            endpoint_label_gap,
            resources,
        )?,
        target_marker_y: checked_add_gap(target_bottom, 1, endpoint_label_gap, resources)?,
        target_path_end_y: checked_add_gap(
            target_bottom,
            request.profile.target_path_end_offset_from_target,
            endpoint_label_gap,
            resources,
        )?,
    })
}

fn route_column_crosses_any_box(
    center_x: usize,
    lane_offset: isize,
    boxes: &[&PlacedRelationGraphBox<'_>],
    resources: &ResourceContext,
) -> Result<bool> {
    if !lane_offset_fits_center(center_x, lane_offset) {
        return Ok(true);
    }
    let column = offset_center(center_x, lane_offset, resources)?;
    Ok(boxes
        .iter()
        .any(|placed_box| column >= placed_box.x() && column <= placed_box.right()))
}

fn lane_offset_fits_center(center_x: usize, lane_offset: isize) -> bool {
    if lane_offset < 0 {
        lane_offset.unsigned_abs() <= center_x
    } else {
        usize::try_from(lane_offset)
            .ok()
            .and_then(|offset| center_x.checked_add(offset))
            .is_some()
    }
}

fn top_is_left_of_intermediate_boxes(
    top: &PlacedRelationGraphBox<'_>,
    intermediate_boxes: &[&PlacedRelationGraphBox<'_>],
) -> bool {
    intermediate_boxes
        .iter()
        .any(|placed_box| top.center_x() < placed_box.center_x())
}

fn route_column_left_of_intermediate_boxes(
    top: &PlacedRelationGraphBox<'_>,
    intermediate_boxes: &[&PlacedRelationGraphBox<'_>],
    resources: &ResourceContext,
) -> Result<isize> {
    let target = intermediate_boxes
        .iter()
        .map(|placed_box| placed_box.x())
        .min()
        .unwrap_or(0)
        .checked_sub(2)
        .ok_or_else(|| grid_overflow(resources))?;
    checked_signed_difference(target, top.center_x(), resources)
}

fn route_column_right_of_intermediate_boxes(
    top: &PlacedRelationGraphBox<'_>,
    intermediate_boxes: &[&PlacedRelationGraphBox<'_>],
    resources: &ResourceContext,
) -> Result<isize> {
    let target = intermediate_boxes
        .iter()
        .map(|placed_box| placed_box.right())
        .max()
        .unwrap_or(top.center_x())
        .checked_add(2)
        .ok_or_else(|| grid_overflow(resources))?;
    checked_signed_difference(target, top.center_x(), resources)
}

fn centered_text_fits(
    center_x: usize,
    y: usize,
    text_width: usize,
    line_count: usize,
    width: usize,
    height: usize,
) -> bool {
    center_x
        .checked_sub(text_width / 2)
        .and_then(|start| start.checked_add(text_width))
        .is_some_and(|end| end <= width)
        && y.checked_add(line_count).is_some_and(|end| end <= height)
}

fn checked_add_gap(
    base: usize,
    offset: usize,
    gap: usize,
    resources: &ResourceContext,
) -> Result<usize> {
    resources.checked_grid_add(resources.checked_grid_add(base, offset)?, gap)
}

fn checked_sub_gap(
    base: usize,
    offset: usize,
    gap: usize,
    resources: &ResourceContext,
) -> Result<usize> {
    base.checked_sub(offset)
        .and_then(|value| value.checked_sub(gap))
        .ok_or_else(|| grid_overflow(resources))
}

fn checked_signed_difference(
    left: usize,
    right: usize,
    resources: &ResourceContext,
) -> Result<isize> {
    if left >= right {
        isize::try_from(left - right).map_err(|_| grid_overflow(resources))
    } else {
        isize::try_from(right - left)
            .ok()
            .and_then(isize::checked_neg)
            .ok_or_else(|| grid_overflow(resources))
    }
}

fn grid_overflow(resources: &ResourceContext) -> AsciiError {
    resources.grid_overflow()
}

fn work_overflow(resources: &ResourceContext) -> AsciiError {
    resources.work_overflow()
}

fn layout_allocation_failed() -> AsciiError {
    AsciiError::allocation_failed(crate::resource::AsciiResourceLimitPhase::LayoutWork.as_str())
}
