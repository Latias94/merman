use super::super::RelationGraphLabel;
use super::boxes::PlacedRelationGraphBox;
use super::draw::{
    RelationLineChars, draw_relation_span_exclusive, draw_relation_span_inclusive,
    put_relation_char, write_centered_relation_label, write_centered_relation_text,
};
use crate::canvas::Canvas;
use crate::color::AsciiColorRole;

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

    pub(crate) fn text(center_x: usize, y: usize, text: String, role: AsciiColorRole) -> Self {
        Self::Text {
            center_x,
            y,
            text,
            role,
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

    fn draw_at(&self, canvas: &mut Canvas) {
        match self {
            RelationOverlay::Glyph { x, y, ch, role } => canvas.set_role(*x, *y, *ch, *role),
            RelationOverlay::Text {
                center_x,
                y,
                text,
                role,
            } => write_centered_relation_text(canvas, *center_x, *y, text, *role),
            RelationOverlay::Label {
                center_x,
                y,
                label,
                role,
            } => write_centered_relation_label(canvas, *center_x, *y, label, *role),
        }
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
    pub(crate) fn from_x(&self) -> usize {
        self.from_x
    }

    pub(crate) fn to_x(&self) -> usize {
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
            return self.source_marker_y.saturating_add(1).min(self.route_y());
        }

        self.source_marker_y.saturating_sub(1).max(self.route_y())
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

    pub(crate) fn draw_at(&self, canvas: &mut Canvas) {
        draw_relation_span_inclusive(
            canvas,
            self.geometry.from_x,
            self.geometry.source_path_start_y,
            self.geometry.route_y,
            self.vertical_char,
            self.relation_chars,
        );
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
                );
            }
        }
        draw_relation_span_exclusive(
            canvas,
            self.geometry.to_x,
            self.geometry.route_y,
            self.geometry.target_path_end_y,
            self.vertical_char,
            self.relation_chars,
        );

        for overlay in &self.overlays {
            overlay.draw_at(canvas);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LayeredRelationRouteProfile {
    min_vertical_gap: usize,
    source_path_start_offset: usize,
    route_y_offset_from_target: usize,
    target_path_end_offset_from_target: usize,
}

impl LayeredRelationRouteProfile {
    pub(crate) const fn new(
        min_vertical_gap: usize,
        source_path_start_offset: usize,
        route_y_offset_from_target: usize,
        target_path_end_offset_from_target: usize,
    ) -> Self {
        Self {
            min_vertical_gap,
            source_path_start_offset,
            route_y_offset_from_target,
            target_path_end_offset_from_target,
        }
    }

    pub(crate) const fn class() -> Self {
        Self::new(1, 1, 1, 0)
    }

    pub(crate) const fn er() -> Self {
        Self::new(2, 2, 2, 1)
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

pub(crate) fn draw_layered_relation_route(
    canvas: &mut Canvas,
    request: LayeredRelationRouteRequest<'_, '_>,
    style: LayeredRelationRouteStyle,
    build_overlays: impl FnOnce(&LayeredRelationRouteGeometry) -> Vec<RelationOverlay>,
) {
    let geometry = plan_layered_relation_route(request);
    let overlays = build_overlays(&geometry);
    LayeredRelationRoutePlan::new(
        geometry,
        style.vertical_char,
        style.horizontal_char,
        style.relation_chars,
        overlays,
    )
    .draw_at(canvas);
}

pub(crate) fn offset_center(center: usize, offset: isize) -> usize {
    if offset < 0 {
        center.saturating_sub(offset.unsigned_abs())
    } else {
        center.saturating_add(offset as usize)
    }
}

pub(crate) fn spanning_lane_offset(top_width: usize, bottom_width: usize) -> isize {
    (top_width.max(bottom_width) / 2).saturating_add(3) as isize
}

pub(crate) fn spanning_lane_offset_around_intermediate_boxes(
    placed_boxes: &[PlacedRelationGraphBox<'_>],
    top: &PlacedRelationGraphBox<'_>,
    bottom: &PlacedRelationGraphBox<'_>,
    lane_offset: isize,
) -> isize {
    let lower_bound = top.y().min(bottom.y());
    let upper_bound = top.bottom().max(bottom.bottom());
    let intermediate_boxes = placed_boxes
        .iter()
        .filter(|placed_box| placed_box.y() > lower_bound && placed_box.bottom() < upper_bound)
        .collect::<Vec<_>>();
    if intermediate_boxes.is_empty() {
        return lane_offset;
    }

    let route_clearance = intermediate_boxes
        .iter()
        .map(|placed_box| placed_box.width() / 2)
        .max()
        .unwrap_or(0);
    let spanning_offset = spanning_lane_offset(
        top.width().max(route_clearance.saturating_mul(2)),
        bottom.width(),
    );
    let left_offset = lane_offset - spanning_offset;
    let right_offset = lane_offset + spanning_offset;
    let left_is_clear =
        !route_column_crosses_any_box(top.center_x(), left_offset, &intermediate_boxes);
    let right_is_clear =
        !route_column_crosses_any_box(top.center_x(), right_offset, &intermediate_boxes);

    match (left_is_clear, right_is_clear) {
        (true, false) => left_offset,
        (false, true) => right_offset,
        (true, true) if lane_offset < 0 => left_offset,
        (true, true) if top_is_left_of_intermediate_boxes(top, &intermediate_boxes) => left_offset,
        (true, true) => right_offset,
        (false, false) if top_is_left_of_intermediate_boxes(top, &intermediate_boxes) => {
            route_column_left_of_intermediate_boxes(top, &intermediate_boxes)
        }
        (false, false) => route_column_right_of_intermediate_boxes(top, &intermediate_boxes),
    }
}

pub(crate) fn plan_layered_relation_route(
    request: LayeredRelationRouteRequest<'_, '_>,
) -> LayeredRelationRouteGeometry {
    let lane_offset = spanning_lane_offset_around_intermediate_boxes(
        request.placed_boxes,
        request.top,
        request.bottom,
        request.lane_offset,
    );
    let from_x = offset_center(request.top.center_x(), lane_offset);
    let to_x = offset_center(request.bottom.center_x(), lane_offset);
    let source_top = request.top.y();
    let source_bottom = request.top.bottom();
    let target_top = request.bottom.y();
    let target_bottom = request.bottom.bottom();

    if target_top > source_bottom.saturating_add(request.profile.min_vertical_gap) {
        return LayeredRelationRouteGeometry {
            from_x,
            to_x,
            source_path_start_y: source_bottom
                .saturating_add(request.profile.source_path_start_offset),
            source_marker_y: source_bottom.saturating_add(1),
            route_y: target_top.saturating_sub(request.profile.route_y_offset_from_target),
            target_marker_y: target_top.saturating_sub(1),
            target_path_end_y: target_top
                .saturating_sub(request.profile.target_path_end_offset_from_target),
        };
    }

    if source_top > target_bottom.saturating_add(request.profile.min_vertical_gap) {
        return LayeredRelationRouteGeometry {
            from_x,
            to_x,
            source_path_start_y: source_top
                .saturating_sub(request.profile.source_path_start_offset),
            source_marker_y: source_top.saturating_sub(1),
            route_y: target_bottom.saturating_add(request.profile.route_y_offset_from_target),
            target_marker_y: target_bottom.saturating_add(1),
            target_path_end_y: target_bottom
                .saturating_add(request.profile.target_path_end_offset_from_target),
        };
    }

    LayeredRelationRouteGeometry {
        from_x,
        to_x,
        source_path_start_y: source_bottom.saturating_add(request.profile.source_path_start_offset),
        source_marker_y: source_bottom.saturating_add(1),
        route_y: source_bottom
            .max(target_bottom)
            .saturating_add(request.profile.route_y_offset_from_target),
        target_marker_y: target_bottom.saturating_add(1),
        target_path_end_y: target_bottom
            .saturating_add(request.profile.target_path_end_offset_from_target),
    }
}

fn route_column_crosses_any_box(
    center_x: usize,
    lane_offset: isize,
    boxes: &[&PlacedRelationGraphBox<'_>],
) -> bool {
    let column = offset_center(center_x, lane_offset);
    boxes
        .iter()
        .any(|placed_box| column >= placed_box.x() && column <= placed_box.right())
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
) -> isize {
    let target = intermediate_boxes
        .iter()
        .map(|placed_box| placed_box.x())
        .min()
        .unwrap_or(0)
        .saturating_sub(2);
    target as isize - top.center_x() as isize
}

fn route_column_right_of_intermediate_boxes(
    top: &PlacedRelationGraphBox<'_>,
    intermediate_boxes: &[&PlacedRelationGraphBox<'_>],
) -> isize {
    let target = intermediate_boxes
        .iter()
        .map(|placed_box| placed_box.right())
        .max()
        .unwrap_or(top.center_x())
        .saturating_add(2);
    target as isize - top.center_x() as isize
}
