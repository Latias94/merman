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

/// Retain label alignment until the family supplies its measured text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelationLabelAnchor {
    Centered(usize),
    LeftAligned(usize),
}

impl RelationLabelAnchor {
    pub(super) fn left(self, width: usize) -> Option<usize> {
        match self {
            Self::Centered(center) => center.checked_sub(width / 2),
            Self::LeftAligned(left) => Some(left),
        }
    }

    fn x_mut(&mut self) -> &mut usize {
        match self {
            Self::Centered(x) | Self::LeftAligned(x) => x,
        }
    }
}

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
        anchor: RelationLabelAnchor,
        y: usize,
        label: RelationGraphLabel,
        role: AsciiColorRole,
        endpoint: bool,
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
            anchor: RelationLabelAnchor::Centered(center_x),
            y,
            label,
            role,
            endpoint: false,
        }
    }

    pub(crate) fn endpoint_label(
        anchor: RelationLabelAnchor,
        y: usize,
        label: RelationGraphLabel,
        role: AsciiColorRole,
    ) -> Self {
        Self::Label {
            anchor,
            y,
            label,
            role,
            endpoint: true,
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
                anchor,
                y,
                label,
                role,
                ..
            } => {
                if let RelationLabelAnchor::Centered(center_x) = anchor {
                    return write_centered_relation_label(canvas, *center_x, *y, label, *role);
                }
                for (offset, line) in label.lines().iter().enumerate() {
                    let Some(row) = y.checked_add(offset) else {
                        return Ok(());
                    };
                    if let Some(left) = anchor.left(line.width()) {
                        canvas.write_deferred_text_role(left, row, line, *role)?;
                    }
                }
                Ok(())
            }
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
                anchor, y, label, ..
            } => {
                anchor
                    .left(label.width())
                    .and_then(|left| left.checked_add(label.width()))
                    .is_some_and(|right| right <= width)
                    && y.checked_add(label.line_count())
                        .is_some_and(|end| end <= height)
            }
        }
    }

    pub(super) fn bounds(&self) -> Option<RelationOverlayBounds> {
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
                anchor, y, label, ..
            } => {
                return RelationOverlayBounds::new(
                    anchor.left(label.width())?,
                    *y,
                    label.width(),
                    label.line_count(),
                );
            }
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
pub(super) struct RelationOverlayBounds {
    pub(super) left: usize,
    pub(super) right: usize,
    pub(super) top: usize,
    pub(super) bottom: usize,
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
    pub(super) side: PhysicalPortSide,
    pub(super) marker_x: usize,
    pub(super) marker_y: usize,
    pub(super) path_x: usize,
    pub(super) path_y: usize,
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

    pub(super) fn private_endpoint_label_anchor(
        self,
        line_count: usize,
        resources: &ResourceContext,
    ) -> Result<(RelationLabelAnchor, usize)> {
        match self.side {
            PhysicalPortSide::Top | PhysicalPortSide::Bottom => Ok((
                RelationLabelAnchor::LeftAligned(resources.checked_grid_add(self.marker_x, 2)?),
                if self.side == PhysicalPortSide::Top {
                    self.marker_y.saturating_sub(line_count.saturating_sub(1))
                } else {
                    self.marker_y
                },
            )),
            PhysicalPortSide::Left | PhysicalPortSide::Right => Ok((
                RelationLabelAnchor::Centered(self.marker_x),
                resources.checked_grid_add(self.marker_y, 2)?,
            )),
        }
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
    fn corner_with(self, other: Self) -> Option<(usize, usize, bool, bool)> {
        let (y, left, right, x, top, bottom) = match (self, other) {
            (Self::Horizontal { y, left, right }, Self::Vertical { x, top, bottom })
            | (Self::Vertical { x, top, bottom }, Self::Horizontal { y, left, right }) => {
                (y, left, right, x, top, bottom)
            }
            _ => return None,
        };
        if left < right && top < bottom && (x == left || x == right) && (y == top || y == bottom) {
            Some((x, y, x == left, y == top))
        } else {
            None
        }
    }

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
    Private { center_x: usize, y: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LayeredRelationRouteGeometry {
    source: LayeredRelationPhysicalPort,
    target: LayeredRelationPhysicalPort,
    segments: Vec<LayeredRelationRouteSegment>,
    label_placement: LayeredRelationLabelPlacement,
}

impl LayeredRelationRouteGeometry {
    /// Private channels must start and finish at the cells touching the actual
    /// boxes. Ordered segments also preserve the terminal normals after merging.
    pub(crate) fn connects_boxes(
        &self,
        source: &PlacedRelationGraphBox<'_>,
        target: &PlacedRelationGraphBox<'_>,
    ) -> bool {
        if !self.source.fits_box(source) || !self.target.fits_box(target) {
            return false;
        }
        let contact = |port: LayeredRelationPhysicalPort, node: &PlacedRelationGraphBox<'_>| {
            match port.side {
                PhysicalPortSide::Top => node.y().checked_sub(1).map(|y| (port.path_x, y)),
                PhysicalPortSide::Bottom => node.bottom().checked_add(1).map(|y| (port.path_x, y)),
                PhysicalPortSide::Left => node.x().checked_sub(1).map(|x| (x, port.path_y)),
                PhysicalPortSide::Right => node.right().checked_add(1).map(|x| (x, port.path_y)),
            }
        };
        let Some(mut current) = contact(self.source, source) else {
            return false;
        };
        let end = contact(self.target, target);
        for (index, segment) in self.segments.iter().enumerate() {
            let (first, last) = match *segment {
                LayeredRelationRouteSegment::Horizontal { y, left, right } => {
                    ((left, y), (right, y))
                }
                LayeredRelationRouteSegment::Vertical { x, top, bottom } => ((x, top), (x, bottom)),
            };
            let next = if current == first {
                last
            } else if current == last {
                first
            } else {
                return false;
            };
            let normal = |from: (usize, usize), to: (usize, usize), side| match side {
                PhysicalPortSide::Top => from.0 == to.0 && from.1 > to.1,
                PhysicalPortSide::Right => from.1 == to.1 && from.0 < to.0,
                PhysicalPortSide::Bottom => from.0 == to.0 && from.1 < to.1,
                PhysicalPortSide::Left => from.1 == to.1 && from.0 > to.0,
            };
            if index == 0 && !normal(current, next, self.source.side) {
                return false;
            }
            if index + 1 == self.segments.len()
                && !normal(current, next, self.target.side.opposite())
            {
                return false;
            }
            current = next;
        }
        !self.segments.is_empty() && Some(current) == end
    }

    pub(super) fn from_channel_path(
        source: LayeredRelationPhysicalPort,
        target: LayeredRelationPhysicalPort,
        path: &[(usize, usize)],
        label_anchor: (usize, usize),
        resources: &ResourceContext,
    ) -> Result<Self> {
        resources.charge_layout_work(path.len().max(1))?;
        let mut segments = Vec::new();
        segments
            .try_reserve_exact(path.len())
            .map_err(|_| layout_allocation_failed())?;
        if let Some(&first) = path.first() {
            let mut start = first;
            let mut previous = first;
            let mut horizontal = None;
            for &point in path.iter().skip(1) {
                resources.checkpoint()?;
                let next_horizontal = point.1 == previous.1;
                if horizontal.is_some_and(|axis| axis != next_horizontal) {
                    segments.push(if horizontal == Some(true) {
                        LayeredRelationRouteSegment::horizontal(start.0, previous.0, start.1)
                    } else {
                        LayeredRelationRouteSegment::vertical(start.0, start.1, previous.1)
                    });
                    start = previous;
                }
                horizontal = Some(next_horizontal);
                previous = point;
            }
            segments.push(if horizontal == Some(true) {
                LayeredRelationRouteSegment::horizontal(start.0, previous.0, start.1)
            } else {
                LayeredRelationRouteSegment::vertical(start.0, start.1, previous.1)
            });
        }
        Ok(Self {
            source,
            target,
            segments,
            label_placement: LayeredRelationLabelPlacement::Private {
                center_x: label_anchor.0,
                y: label_anchor.1,
            },
        })
    }

    pub(super) fn set_private_label_anchor(&mut self, center_x: usize, y: usize) {
        self.label_placement = LayeredRelationLabelPlacement::Private { center_x, y };
    }

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
            LayeredRelationLabelPlacement::Private { y, .. } => y,
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
        let _ = line_count;
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
            LayeredRelationLabelPlacement::Private { center_x, y } => Ok((center_x, y)),
        }
    }

    pub(crate) fn endpoint_label_anchor(
        &self,
        source: bool,
        line_count: usize,
        resources: &ResourceContext,
    ) -> Result<(RelationLabelAnchor, usize)> {
        let port = if source { self.source } else { self.target };
        if matches!(
            self.label_placement,
            LayeredRelationLabelPlacement::Private { .. }
        ) {
            return port.private_endpoint_label_anchor(line_count, resources);
        }
        let y = match port.side {
            PhysicalPortSide::Top => resources.checked_grid_add(port.marker_y, 1)?,
            PhysicalPortSide::Bottom => port
                .marker_y
                .checked_sub(line_count)
                .ok_or_else(|| grid_overflow(resources))?,
            PhysicalPortSide::Left | PhysicalPortSide::Right => {
                resources.checked_grid_add(port.marker_y, 2)?
            }
        };
        Ok((RelationLabelAnchor::Centered(port.marker_x), y))
    }

    fn segments(&self) -> impl Iterator<Item = LayeredRelationRouteSegment> + '_ {
        self.segments.iter().copied()
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
        ) && self.source.path_x == self.target.path_x
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
    pub(super) fn retained_compaction_cells(&self, resources: &ResourceContext) -> Result<usize> {
        resources.checked_grid_add(self.geometry.segments.capacity(), self.overlays.capacity())
    }

    pub(super) fn protect_compaction_features(
        &self,
        x: &mut super::compact::CompactAxis,
        y: &mut super::compact::CompactAxis,
        resources: &ResourceContext,
    ) -> Result<()> {
        for port in [self.geometry.source, self.geometry.target] {
            x.protect(port.marker_x, port.marker_x + 1, resources)?;
            y.protect(port.marker_y, port.marker_y + 1, resources)?;
        }
        for segment in self.geometry.segments() {
            resources.checkpoint()?;
            let (left, top, right, bottom) = match segment {
                LayeredRelationRouteSegment::Horizontal { y, left, right } => (left, y, right, y),
                LayeredRelationRouteSegment::Vertical { x, top, bottom } => (x, top, x, bottom),
            };
            for coordinate in [left, right] {
                x.protect(coordinate, coordinate + 1, resources)?;
            }
            for coordinate in [top, bottom] {
                y.protect(coordinate, coordinate + 1, resources)?;
            }
        }
        for overlay in &self.overlays {
            resources.checkpoint()?;
            if let Some(bounds) = overlay.bounds() {
                x.protect(bounds.left, bounds.right, resources)?;
                y.protect(bounds.top, bounds.bottom, resources)?;
            }
        }
        Ok(())
    }

    pub(super) fn compact_axes(
        &mut self,
        x: &super::compact::CompactAxis,
        y: &super::compact::CompactAxis,
        resources: &ResourceContext,
    ) -> Result<()> {
        resources.charge_layout_work(self.segment_count() + self.overlay_count() + 2)?;
        for port in [&mut self.geometry.source, &mut self.geometry.target] {
            port.marker_x = x.map(port.marker_x);
            port.path_x = x.map(port.path_x);
            port.marker_y = y.map(port.marker_y);
            port.path_y = y.map(port.path_y);
        }
        for segment in &mut self.geometry.segments {
            match segment {
                LayeredRelationRouteSegment::Horizontal {
                    y: row,
                    left,
                    right,
                } => {
                    *row = y.map(*row);
                    *left = x.map(*left);
                    *right = x.map(*right);
                }
                LayeredRelationRouteSegment::Vertical {
                    x: col,
                    top,
                    bottom,
                } => {
                    *col = x.map(*col);
                    *top = y.map(*top);
                    *bottom = y.map(*bottom);
                }
            }
        }
        match &mut self.geometry.label_placement {
            LayeredRelationLabelPlacement::LegacyAfterSource { route_y } => {
                *route_y = y.map(*route_y)
            }
            LayeredRelationLabelPlacement::Private { center_x, y: row } => {
                *center_x = x.map_unused_anchor(*center_x);
                *row = y.map_unused_anchor(*row);
            }
        }
        for overlay in &mut self.overlays {
            match overlay {
                RelationOverlay::Glyph { x: col, y: row, .. } => {
                    *col = x.map(*col);
                    *row = y.map(*row);
                }
                RelationOverlay::Text {
                    center_x, y: row, ..
                } => {
                    *center_x = x.map(*center_x);
                    *row = y.map(*row);
                }
                RelationOverlay::Label { anchor, y: row, .. } => {
                    let col = anchor.x_mut();
                    *col = x.map(*col);
                    *row = y.map(*row);
                }
            }
        }
        Ok(())
    }

    pub(super) fn bounds(&self) -> (usize, usize, usize, usize) {
        let source = self.geometry.source;
        let target = self.geometry.target;
        let mut left = source.marker_x.min(target.marker_x);
        let mut top = source.marker_y.min(target.marker_y);
        let mut right = source.marker_x.max(target.marker_x) + 1;
        let mut bottom = source.marker_y.max(target.marker_y) + 1;
        for segment in self.geometry.segments() {
            let (x0, y0, x1, y1) = match segment {
                LayeredRelationRouteSegment::Horizontal { y, left, right } => {
                    (left, y, right + 1, y + 1)
                }
                LayeredRelationRouteSegment::Vertical { x, top, bottom } => {
                    (x, top, x + 1, bottom + 1)
                }
            };
            left = left.min(x0);
            top = top.min(y0);
            right = right.max(x1);
            bottom = bottom.max(y1);
        }
        for overlay in &self.overlays {
            if let Some(bounds) = overlay.bounds() {
                left = left.min(bounds.left);
                top = top.min(bounds.top);
                right = right.max(bounds.right);
                bottom = bottom.max(bounds.bottom);
            }
        }
        (left, top, right, bottom)
    }

    pub(super) fn translate_to_origin(&mut self, left: usize, top: usize) {
        for port in [&mut self.geometry.source, &mut self.geometry.target] {
            port.marker_x -= left;
            port.path_x -= left;
            port.marker_y -= top;
            port.path_y -= top;
        }
        for segment in &mut self.geometry.segments {
            match segment {
                LayeredRelationRouteSegment::Horizontal {
                    y,
                    left: start,
                    right,
                } => {
                    *y -= top;
                    *start -= left;
                    *right -= left;
                }
                LayeredRelationRouteSegment::Vertical {
                    x,
                    top: start,
                    bottom,
                } => {
                    *x -= left;
                    *start -= top;
                    *bottom -= top;
                }
            }
        }
        match &mut self.geometry.label_placement {
            LayeredRelationLabelPlacement::LegacyAfterSource { route_y } => *route_y -= top,
            // An unused label anchor is metadata and may lie outside painted
            // bounds. Real overlays below remain strictly translated.
            LayeredRelationLabelPlacement::Private { center_x, y } => {
                *center_x = center_x.saturating_sub(left);
                *y = y.saturating_sub(top);
            }
        }
        for overlay in &mut self.overlays {
            match overlay {
                RelationOverlay::Glyph { x, y, .. } => {
                    *x -= left;
                    *y -= top;
                }
                RelationOverlay::Text { center_x, y, .. } => {
                    *center_x -= left;
                    *y -= top;
                }
                RelationOverlay::Label { anchor, y, .. } => {
                    *anchor.x_mut() -= left;
                    *y -= top;
                }
            }
        }
    }

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
        if matches!(
            self.geometry.label_placement,
            LayeredRelationLabelPlacement::Private { .. }
        ) {
            resources.charge_layout_work(self.geometry.segments.len())?;
            for pair in self.geometry.segments.windows(2) {
                checkpoints.tick(resources)?;
                if let Some((x, y, east, south)) = pair[0].corner_with(pair[1]) {
                    canvas.try_set_role(
                        x,
                        y,
                        self.relation_chars.private_corner(east, south),
                        AsciiColorRole::EdgeLine,
                    )?;
                }
            }
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
    pub(super) min_vertical_gap: usize,
    pub(super) source_path_start_offset: usize,
    pub(super) route_y_offset_from_target: usize,
    pub(super) target_path_end_offset_from_target: usize,
    pub(super) endpoint_label_gap: usize,
    pub(super) source_marker_widths: [usize; 2],
    pub(super) target_marker_widths: [usize; 2],
    pub(super) endpoint_label_sizes: [Option<(usize, usize)>; 2],
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
            source_marker_widths: [1, 2],
            target_marker_widths: [1, 2],
            endpoint_label_sizes: [None; 2],
        }
    }

    pub(crate) const fn class() -> Self {
        Self::new(1, 1, 1, 0, 0)
    }

    pub(crate) const fn class_with_endpoint_labels(endpoint_label_gap: usize) -> Self {
        Self::new(1, 1, 1, 0, endpoint_label_gap)
    }

    pub(crate) const fn er() -> Self {
        Self::new(2, 2, 2, 1, 0).with_marker_widths([2, 2], [2, 2])
    }

    pub(crate) const fn with_marker_widths(
        mut self,
        source: [usize; 2],
        target: [usize; 2],
    ) -> Self {
        self.source_marker_widths = source;
        self.target_marker_widths = target;
        self
    }

    #[cfg(feature = "diagram-class")]
    pub(crate) const fn with_endpoint_label_sizes(
        mut self,
        sizes: [Option<(usize, usize)>; 2],
    ) -> Self {
        self.endpoint_label_sizes = sizes;
        self
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

fn three_segment_geometry(
    source: LayeredRelationPhysicalPort,
    target: LayeredRelationPhysicalPort,
    route_y: usize,
    label_placement: LayeredRelationLabelPlacement,
) -> LayeredRelationRouteGeometry {
    let mut segments = vec![LayeredRelationRouteSegment::vertical(
        source.path_x,
        source.path_y,
        route_y,
    )];
    if source.path_x != target.path_x {
        segments.push(LayeredRelationRouteSegment::horizontal(
            source.path_x,
            target.path_x,
            route_y,
        ));
    }
    segments.push(LayeredRelationRouteSegment::vertical(
        target.path_x,
        route_y,
        target.path_y,
    ));
    LayeredRelationRouteGeometry {
        source,
        target,
        segments,
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
