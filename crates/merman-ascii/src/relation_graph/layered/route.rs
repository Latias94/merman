use super::super::{PhysicalPortSide, RelationGraphLabel, RelationResourceCheckpointCursor};
use super::boxes::PlacedRelationGraphBox;
use super::draw::{
    RelationLineChars, put_relation_char, write_centered_relation_label,
    write_centered_relation_text,
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

    pub(crate) fn fits(&self, width: usize, height: usize) -> bool {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayeredRelationPhysicalPort {
    side: PhysicalPortSide,
    marker_x: usize,
    marker_y: usize,
    path_x: usize,
    path_y: usize,
}

impl LayeredRelationPhysicalPort {
    pub(crate) fn side(self) -> PhysicalPortSide {
        self.side
    }

    pub(crate) fn x(self) -> usize {
        self.marker_x
    }

    pub(crate) fn marker_y(self) -> usize {
        self.marker_y
    }

    pub(crate) fn fits_box(self, relation_box: &PlacedRelationGraphBox<'_>) -> bool {
        match self.side {
            PhysicalPortSide::Top => {
                (relation_box.x()..=relation_box.right()).contains(&self.marker_x)
                    && self.marker_y < relation_box.y()
                    && self.path_y < relation_box.y()
            }
            PhysicalPortSide::Bottom => {
                (relation_box.x()..=relation_box.right()).contains(&self.marker_x)
                    && self.marker_y > relation_box.bottom()
                    && self.path_y > relation_box.bottom()
            }
            PhysicalPortSide::Left => {
                (relation_box.y()..=relation_box.bottom()).contains(&self.marker_y)
                    && self.marker_x < relation_box.x()
                    && self.path_x < relation_box.x()
            }
            PhysicalPortSide::Right => {
                (relation_box.y()..=relation_box.bottom()).contains(&self.marker_y)
                    && self.marker_x > relation_box.right()
                    && self.path_x > relation_box.right()
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayeredRelationRouteSegment {
    Horizontal { y: usize, left: usize, right: usize },
    Vertical { x: usize, top: usize, bottom: usize },
}

impl LayeredRelationRouteSegment {
    fn horizontal(start_x: usize, end_x: usize, y: usize) -> Self {
        Self::Horizontal {
            y,
            left: start_x.min(end_x),
            right: start_x.max(end_x),
        }
    }

    fn vertical(x: usize, start_y: usize, end_y: usize) -> Self {
        Self::Vertical {
            x,
            top: start_y.min(end_y),
            bottom: start_y.max(end_y),
        }
    }

    fn draw(
        self,
        canvas: &mut Canvas,
        vertical_char: char,
        horizontal_char: char,
        relation_chars: RelationLineChars,
        resources: &ResourceContext,
        checkpoints: &mut RelationResourceCheckpointCursor,
    ) -> Result<()> {
        match self {
            Self::Horizontal { y, left, right } => {
                for x in left..=right {
                    checkpoints.tick(resources)?;
                    put_relation_char(canvas, x, y, horizontal_char, relation_chars)?;
                }
                Ok(())
            }
            Self::Vertical { x, top, bottom } => {
                for y in top..=bottom {
                    checkpoints.tick(resources)?;
                    put_relation_char(canvas, x, y, vertical_char, relation_chars)?;
                }
                Ok(())
            }
        }
    }

    fn fits(self, width: usize, height: usize) -> bool {
        match self {
            Self::Horizontal { y, left, right } => right < width && left <= right && y < height,
            Self::Vertical { x, top, bottom } => x < width && bottom < height && top <= bottom,
        }
    }

    fn cell_count(self) -> Option<usize> {
        match self {
            Self::Horizontal { left, right, .. } => right.checked_sub(left)?.checked_add(1),
            Self::Vertical { top, bottom, .. } => bottom.checked_sub(top)?.checked_add(1),
        }
    }

    fn overlaps_rect(self, left: usize, top: usize, right: usize, bottom: usize) -> bool {
        match self {
            Self::Horizontal {
                y,
                left: segment_left,
                right: segment_right,
            } => (top..=bottom).contains(&y) && segment_left <= right && left <= segment_right,
            Self::Vertical {
                x,
                top: segment_top,
                bottom: segment_bottom,
            } => (left..=right).contains(&x) && segment_top <= bottom && top <= segment_bottom,
        }
    }

    fn overlaps(self, other: Self) -> bool {
        match (self, other) {
            (
                Self::Horizontal {
                    y: left_y,
                    left: left_start,
                    right: left_end,
                },
                Self::Horizontal {
                    y: right_y,
                    left: right_start,
                    right: right_end,
                },
            ) => left_y == right_y && left_start <= right_end && right_start <= left_end,
            (
                Self::Vertical {
                    x: left_x,
                    top: left_start,
                    bottom: left_end,
                },
                Self::Vertical {
                    x: right_x,
                    top: right_start,
                    bottom: right_end,
                },
            ) => left_x == right_x && left_start <= right_end && right_start <= left_end,
            (Self::Horizontal { y, left, right }, Self::Vertical { x, top, bottom })
            | (Self::Vertical { x, top, bottom }, Self::Horizontal { y, left, right }) => {
                (left..=right).contains(&x) && (top..=bottom).contains(&y)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayeredRelationLabelPlacement {
    LegacyAfterSource { route_y: usize },
    TopLane { center_x: usize, y: usize },
    BottomLane { center_x: usize, y: usize },
    Vertical { center_x: usize, center_y: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayeredRelationRouteGeometry {
    source: LayeredRelationPhysicalPort,
    target: LayeredRelationPhysicalPort,
    segments: [Option<LayeredRelationRouteSegment>; 3],
    label_placement: LayeredRelationLabelPlacement,
}

impl LayeredRelationRouteGeometry {
    pub(crate) fn source_port(&self) -> LayeredRelationPhysicalPort {
        self.source
    }

    pub(crate) fn target_port(&self) -> LayeredRelationPhysicalPort {
        self.target
    }

    pub(crate) fn source_x(&self) -> usize {
        self.source.x()
    }

    pub(crate) fn target_x(&self) -> usize {
        self.target.x()
    }

    pub(crate) fn source_marker_y(&self) -> usize {
        self.source.marker_y()
    }

    pub(crate) fn target_marker_y(&self) -> usize {
        self.target.marker_y()
    }

    #[cfg(test)]
    pub(crate) fn route_y(&self) -> usize {
        match self.label_placement {
            LayeredRelationLabelPlacement::LegacyAfterSource { route_y } => route_y,
            LayeredRelationLabelPlacement::TopLane { y, .. }
            | LayeredRelationLabelPlacement::BottomLane { y, .. } => y,
            LayeredRelationLabelPlacement::Vertical { center_y, .. } => center_y,
        }
    }

    #[cfg(test)]
    pub(crate) fn label_y_after_source(&self) -> usize {
        if self.source.marker_y <= self.target.marker_y {
            return self
                .source
                .marker_y
                .checked_add(1)
                .unwrap_or_else(|| self.route_y())
                .min(self.route_y());
        }

        self.source
            .marker_y
            .checked_sub(1)
            .unwrap_or_else(|| self.route_y())
            .max(self.route_y())
    }

    pub(crate) fn relation_label_anchor(
        &self,
        line_count: usize,
        resources: &ResourceContext,
    ) -> Result<(usize, usize)> {
        let line_count = line_count.max(1);
        match self.label_placement {
            LayeredRelationLabelPlacement::LegacyAfterSource { route_y } => {
                let y = if self.source.marker_y <= self.target.marker_y {
                    resources
                        .checked_grid_add(self.source.marker_y, 1)?
                        .min(route_y)
                } else {
                    self.source.marker_y.saturating_sub(1).max(route_y)
                };
                Ok((
                    resources.checked_grid_add(self.source.x(), self.target.x())? / 2,
                    y,
                ))
            }
            LayeredRelationLabelPlacement::TopLane { center_x, y } => Ok((center_x, y)),
            LayeredRelationLabelPlacement::BottomLane { center_x, y } => Ok((
                center_x,
                y.checked_add(1)
                    .and_then(|bottom| bottom.checked_sub(line_count))
                    .ok_or_else(|| grid_overflow(resources))?,
            )),
            LayeredRelationLabelPlacement::Vertical { center_x, center_y } => Ok((
                center_x,
                center_y.saturating_sub(line_count.saturating_sub(1) / 2),
            )),
        }
    }

    pub(crate) fn endpoint_label_anchor(
        &self,
        source: bool,
        line_count: usize,
        resources: &ResourceContext,
    ) -> Result<(usize, usize)> {
        let port = if source { self.source } else { self.target };
        let y = match port.side {
            PhysicalPortSide::Top => resources.checked_grid_add(port.marker_y, 1)?,
            PhysicalPortSide::Bottom => port
                .marker_y
                .checked_sub(line_count)
                .ok_or_else(|| grid_overflow(resources))?,
            PhysicalPortSide::Left | PhysicalPortSide::Right => port.marker_y,
        };
        Ok((port.marker_x, y))
    }

    fn segments(&self) -> impl Iterator<Item = LayeredRelationRouteSegment> + '_ {
        self.segments.iter().flatten().copied()
    }

    pub(crate) fn segment_count(&self) -> usize {
        self.segments().count()
    }

    fn work(&self, resources: &ResourceContext) -> Result<usize> {
        let segment_work = self.segments().try_fold(0usize, |work, segment| {
            work.checked_add(
                segment
                    .cell_count()
                    .ok_or_else(|| work_overflow(resources))?,
            )
            .ok_or_else(|| work_overflow(resources))
        })?;
        if matches!(
            self.label_placement,
            LayeredRelationLabelPlacement::LegacyAfterSource { .. }
        ) && self.segments[1].is_none()
        {
            return resources.checked_work_add(segment_work, 1);
        }
        Ok(segment_work)
    }

    pub(crate) fn fits(&self, width: usize, height: usize) -> bool {
        self.segments().all(|segment| segment.fits(width, height))
    }

    pub(crate) fn overlaps_rect(
        &self,
        left: usize,
        top: usize,
        right: usize,
        bottom: usize,
    ) -> bool {
        self.segments()
            .any(|segment| segment.overlaps_rect(left, top, right, bottom))
    }

    pub(crate) fn overlaps(&self, other: &Self) -> bool {
        self.segments()
            .any(|left| other.segments().any(|right| left.overlaps(right)))
    }

    fn overlay_overlaps_route(&self, overlay: &RelationOverlay) -> bool {
        overlay.bounds().is_some_and(|bounds| {
            let right = bounds.right.saturating_sub(1);
            let bottom = bounds.bottom.saturating_sub(1);
            self.overlaps_rect(bounds.left, bounds.top, right, bottom)
        })
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

    pub(crate) fn draw_route_at(
        &self,
        canvas: &mut Canvas,
        resources: &ResourceContext,
    ) -> Result<()> {
        let mut checkpoints = RelationResourceCheckpointCursor::new();
        for segment in self.geometry.segments() {
            checkpoints.tick(resources)?;
            segment.draw(
                canvas,
                self.vertical_char,
                self.horizontal_char,
                self.relation_chars,
                resources,
                &mut checkpoints,
            )?;
        }
        Ok(())
    }

    pub(crate) fn geometry(&self) -> &LayeredRelationRouteGeometry {
        &self.geometry
    }

    pub(crate) fn draw_overlays_at(
        &self,
        canvas: &mut Canvas,
        resources: &ResourceContext,
    ) -> Result<()> {
        let mut checkpoints = RelationResourceCheckpointCursor::new();
        for overlay in &self.overlays {
            checkpoints.tick(resources)?;
            overlay.draw_at(canvas)?;
        }
        Ok(())
    }

    pub(crate) fn route_fits(&self, width: usize, height: usize) -> bool {
        self.geometry.fits(width, height)
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

    pub(crate) fn route_overlaps(&self, other: &Self) -> bool {
        self.geometry.overlaps(&other.geometry)
    }

    pub(crate) fn overlays_overlap_route(&self, other: &Self) -> bool {
        self.overlays
            .iter()
            .any(|overlay| other.geometry.overlay_overlaps_route(overlay))
    }

    pub(crate) fn segment_count(&self) -> usize {
        self.geometry.segment_count()
    }

    pub(crate) fn overlay_count(&self) -> usize {
        self.overlays.len()
    }
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

pub(crate) fn plan_layered_relation_route_geometry(
    request: LayeredRelationRouteRequest<'_, '_>,
    resources: &ResourceContext,
) -> Result<LayeredRelationRouteGeometry> {
    resources.charge_layout_work(request.placed_boxes.len().max(1))?;
    let geometry = plan_layered_relation_route(request, resources)?;
    resources.charge_layout_work(geometry.work(resources)?)?;
    Ok(geometry)
}

pub(crate) fn plan_planar_k2_2_relation_route_geometry(
    source: &PlacedRelationGraphBox<'_>,
    target: &PlacedRelationGraphBox<'_>,
    scene_height: usize,
    profile: LayeredRelationRouteProfile,
    resources: &ResourceContext,
) -> Result<Option<LayeredRelationRouteGeometry>> {
    resources.charge_layout_work(2)?;
    let Some(geometry) =
        plan_planar_k2_2_relation_route(source, target, scene_height, profile, resources)?
    else {
        return Ok(None);
    };
    resources.charge_layout_work(geometry.work(resources)?)?;
    Ok(Some(geometry))
}

pub(crate) fn materialize_layered_relation_route_plan(
    geometry: LayeredRelationRouteGeometry,
    style: LayeredRelationRouteStyle,
    resources: &ResourceContext,
    overlays: Vec<RelationOverlay>,
) -> Result<LayeredRelationRoutePlan> {
    resources.charge_layout_work(overlays.len().max(1))?;
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
    resources: &ResourceContext,
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
    resources: &ResourceContext,
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
        let source = LayeredRelationPhysicalPort {
            side: PhysicalPortSide::Bottom,
            marker_x: from_x,
            marker_y: checked_add_gap(source_bottom, 1, endpoint_label_gap, resources)?,
            path_x: from_x,
            path_y: checked_add_gap(
                source_bottom,
                request.profile.source_path_start_offset,
                endpoint_label_gap,
                resources,
            )?,
        };
        let target_boundary_y = checked_sub_gap(
            target_top,
            request.profile.target_path_end_offset_from_target,
            endpoint_label_gap,
            resources,
        )?;
        let target = LayeredRelationPhysicalPort {
            side: PhysicalPortSide::Top,
            marker_x: to_x,
            marker_y: checked_sub_gap(target_top, 1, endpoint_label_gap, resources)?,
            path_x: to_x,
            path_y: target_boundary_y
                .checked_sub(1)
                .ok_or_else(|| grid_overflow(resources))?,
        };
        let route_y = checked_sub_gap(
            target_top,
            request.profile.route_y_offset_from_target,
            endpoint_label_gap,
            resources,
        )?;
        return Ok(three_segment_geometry(
            source,
            target,
            route_y,
            LayeredRelationLabelPlacement::LegacyAfterSource { route_y },
        ));
    }

    if source_top > resources.checked_grid_add(target_bottom, request.profile.min_vertical_gap)? {
        let source = LayeredRelationPhysicalPort {
            side: PhysicalPortSide::Top,
            marker_x: from_x,
            marker_y: checked_sub_gap(source_top, 1, endpoint_label_gap, resources)?,
            path_x: from_x,
            path_y: checked_sub_gap(
                source_top,
                request.profile.source_path_start_offset,
                endpoint_label_gap,
                resources,
            )?,
        };
        let target_boundary_y = checked_add_gap(
            target_bottom,
            request.profile.target_path_end_offset_from_target,
            endpoint_label_gap,
            resources,
        )?;
        let target = LayeredRelationPhysicalPort {
            side: PhysicalPortSide::Bottom,
            marker_x: to_x,
            marker_y: checked_add_gap(target_bottom, 1, endpoint_label_gap, resources)?,
            path_x: to_x,
            path_y: resources.checked_grid_add(target_boundary_y, 1)?,
        };
        let route_y = checked_add_gap(
            target_bottom,
            request.profile.route_y_offset_from_target,
            endpoint_label_gap,
            resources,
        )?;
        return Ok(three_segment_geometry(
            source,
            target,
            route_y,
            LayeredRelationLabelPlacement::LegacyAfterSource { route_y },
        ));
    }

    let source = LayeredRelationPhysicalPort {
        side: PhysicalPortSide::Bottom,
        marker_x: from_x,
        marker_y: checked_add_gap(source_bottom, 1, endpoint_label_gap, resources)?,
        path_x: from_x,
        path_y: checked_add_gap(
            source_bottom,
            request.profile.source_path_start_offset,
            endpoint_label_gap,
            resources,
        )?,
    };
    let target_boundary_y = checked_add_gap(
        target_bottom,
        request.profile.target_path_end_offset_from_target,
        endpoint_label_gap,
        resources,
    )?;
    let target = LayeredRelationPhysicalPort {
        side: PhysicalPortSide::Bottom,
        marker_x: to_x,
        marker_y: checked_add_gap(target_bottom, 1, endpoint_label_gap, resources)?,
        path_x: to_x,
        path_y: resources.checked_grid_add(target_boundary_y, 1)?,
    };
    let route_y = checked_add_gap(
        source_bottom.max(target_bottom),
        request.profile.route_y_offset_from_target,
        endpoint_label_gap,
        resources,
    )?;
    Ok(three_segment_geometry(
        source,
        target,
        route_y,
        LayeredRelationLabelPlacement::LegacyAfterSource { route_y },
    ))
}

fn plan_planar_k2_2_relation_route(
    source_box: &PlacedRelationGraphBox<'_>,
    target_box: &PlacedRelationGraphBox<'_>,
    scene_height: usize,
    profile: LayeredRelationRouteProfile,
    resources: &ResourceContext,
) -> Result<Option<LayeredRelationRouteGeometry>> {
    if source_box.y() == target_box.y() {
        let (side, route_y, placement) = if source_box.y() < scene_height / 2 {
            (
                PhysicalPortSide::Top,
                0,
                LayeredRelationLabelPlacement::TopLane {
                    center_x: resources
                        .checked_grid_add(source_box.center_x(), target_box.center_x())?
                        / 2,
                    y: 0,
                },
            )
        } else {
            let route_y = scene_height
                .checked_sub(1)
                .ok_or_else(|| grid_overflow(resources))?;
            (
                PhysicalPortSide::Bottom,
                route_y,
                LayeredRelationLabelPlacement::BottomLane {
                    center_x: resources
                        .checked_grid_add(source_box.center_x(), target_box.center_x())?
                        / 2,
                    y: route_y,
                },
            )
        };
        let source = physical_port(source_box, side, true, profile, resources)?;
        let target = physical_port(target_box, side, false, profile, resources)?;
        return Ok(Some(three_segment_geometry(
            source, target, route_y, placement,
        )));
    }

    let (source_side, target_side) = if source_box.y() < target_box.y() {
        (PhysicalPortSide::Bottom, PhysicalPortSide::Top)
    } else {
        (PhysicalPortSide::Top, PhysicalPortSide::Bottom)
    };
    let source = physical_port(source_box, source_side, true, profile, resources)?;
    let target = physical_port(target_box, target_side, false, profile, resources)?;
    if source.path_x != target.path_x {
        return Ok(None);
    }
    let center_y = source
        .path_y
        .checked_add(target.path_y)
        .ok_or_else(|| grid_overflow(resources))?
        / 2;
    Ok(Some(LayeredRelationRouteGeometry {
        source,
        target,
        segments: [
            Some(LayeredRelationRouteSegment::vertical(
                source.path_x,
                source.path_y,
                target.path_y,
            )),
            None,
            None,
        ],
        label_placement: LayeredRelationLabelPlacement::Vertical {
            center_x: source.path_x,
            center_y,
        },
    }))
}

fn physical_port(
    relation_box: &PlacedRelationGraphBox<'_>,
    side: PhysicalPortSide,
    source: bool,
    profile: LayeredRelationRouteProfile,
    resources: &ResourceContext,
) -> Result<LayeredRelationPhysicalPort> {
    debug_assert!(PhysicalPortSide::ALL.contains(&side));
    let gap = profile.endpoint_label_gap;
    let marker_offset = resources.checked_grid_add(1, gap)?;
    let path_offset = if source {
        resources.checked_grid_add(profile.source_path_start_offset, gap)?
    } else {
        resources.checked_grid_add(
            resources.checked_grid_add(profile.target_path_end_offset_from_target, gap)?,
            1,
        )?
    };
    match side {
        PhysicalPortSide::Top => Ok(LayeredRelationPhysicalPort {
            side,
            marker_x: relation_box.center_x(),
            marker_y: relation_box
                .y()
                .checked_sub(marker_offset)
                .ok_or_else(|| grid_overflow(resources))?,
            path_x: relation_box.center_x(),
            path_y: relation_box
                .y()
                .checked_sub(path_offset)
                .ok_or_else(|| grid_overflow(resources))?,
        }),
        PhysicalPortSide::Bottom => Ok(LayeredRelationPhysicalPort {
            side,
            marker_x: relation_box.center_x(),
            marker_y: resources.checked_grid_add(relation_box.bottom(), marker_offset)?,
            path_x: relation_box.center_x(),
            path_y: resources.checked_grid_add(relation_box.bottom(), path_offset)?,
        }),
        PhysicalPortSide::Left => Ok(LayeredRelationPhysicalPort {
            side,
            marker_x: relation_box
                .x()
                .checked_sub(marker_offset)
                .ok_or_else(|| grid_overflow(resources))?,
            marker_y: resources.checked_grid_add(relation_box.y(), relation_box.height() / 2)?,
            path_x: relation_box
                .x()
                .checked_sub(path_offset)
                .ok_or_else(|| grid_overflow(resources))?,
            path_y: resources.checked_grid_add(relation_box.y(), relation_box.height() / 2)?,
        }),
        PhysicalPortSide::Right => Ok(LayeredRelationPhysicalPort {
            side,
            marker_x: resources.checked_grid_add(relation_box.right(), marker_offset)?,
            marker_y: resources.checked_grid_add(relation_box.y(), relation_box.height() / 2)?,
            path_x: resources.checked_grid_add(relation_box.right(), path_offset)?,
            path_y: resources.checked_grid_add(relation_box.y(), relation_box.height() / 2)?,
        }),
    }
}

fn three_segment_geometry(
    source: LayeredRelationPhysicalPort,
    target: LayeredRelationPhysicalPort,
    route_y: usize,
    label_placement: LayeredRelationLabelPlacement,
) -> LayeredRelationRouteGeometry {
    let horizontal = (source.path_x != target.path_x)
        .then(|| LayeredRelationRouteSegment::horizontal(source.path_x, target.path_x, route_y));
    LayeredRelationRouteGeometry {
        source,
        target,
        segments: [
            Some(LayeredRelationRouteSegment::vertical(
                source.path_x,
                source.path_y,
                route_y,
            )),
            horizontal,
            Some(LayeredRelationRouteSegment::vertical(
                target.path_x,
                route_y,
                target.path_y,
            )),
        ],
        label_placement,
    }
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
    let left = intermediate_boxes
        .iter()
        .map(|placed_box| placed_box.x())
        .min()
        .unwrap_or(0);
    let Some(target) = left.checked_sub(2) else {
        return route_column_right_of_intermediate_boxes(top, intermediate_boxes, resources);
    };
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
