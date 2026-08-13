#[derive(Debug, Clone, Copy)]
pub(super) struct VerticalOwnershipSpan {
    pub(super) node_index: usize,
    pub(super) x: usize,
    pub(super) top: usize,
    pub(super) bottom: usize,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct HorizontalEdgeGeometry {
    pub(super) lane_y: usize,
    pub(super) left_x: usize,
    pub(super) right_x: usize,
    pub(super) left_port_y: usize,
    pub(super) right_port_y: usize,
    pub(super) left_stem: VerticalOwnershipSpan,
    pub(super) right_stem: VerticalOwnershipSpan,
}

/// Records the physical endpoints that two edges can safely share.
///
/// Compatibility belongs to an endpoint rather than the whole relation: a marker on the remote
/// lane must not contaminate ownership of the shared stem. Two edges can share at most two
/// physical endpoints, so a fixed-size representation avoids allocation in the O(E²) scan.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct CompatibleSharedEndpoints {
    first: Option<usize>,
    second: Option<usize>,
}

impl CompatibleSharedEndpoints {
    #[cfg(test)]
    pub(super) fn single(node_index: usize) -> Self {
        Self {
            first: Some(node_index),
            second: None,
        }
    }

    pub(super) fn add(mut self, node_index: usize) -> Self {
        if self.contains(node_index) {
            return self;
        }
        if self.first.is_none() {
            self.first = Some(node_index);
        } else if self.second.is_none() {
            self.second = Some(node_index);
        }
        self
    }

    fn contains(self, node_index: usize) -> bool {
        self.first == Some(node_index) || self.second == Some(node_index)
    }
}

impl HorizontalEdgeGeometry {
    fn vertical_stems(self) -> [VerticalOwnershipSpan; 2] {
        [self.left_stem, self.right_stem]
    }

    pub(super) fn has_owner_collision_with(
        self,
        other: Self,
        compatible_shared_endpoints: CompatibleSharedEndpoints,
    ) -> bool {
        self.vertical_stems().into_iter().any(|stem| {
            other.horizontal_conflicts_with_vertical(stem, compatible_shared_endpoints)
                || other.vertical_stems().into_iter().any(|other_stem| {
                    vertical_stems_conflict(stem, other_stem, compatible_shared_endpoints)
                })
        }) || other
            .vertical_stems()
            .into_iter()
            .any(|stem| self.horizontal_conflicts_with_vertical(stem, compatible_shared_endpoints))
    }

    fn horizontal_conflicts_with_vertical(
        self,
        stem: VerticalOwnershipSpan,
        compatible_shared_endpoints: CompatibleSharedEndpoints,
    ) -> bool {
        self.horizontal_owns_vertical(stem)
            && !(compatible_shared_endpoints.contains(stem.node_index)
                && self.owns_endpoint(stem.node_index, stem.x))
    }

    fn horizontal_owns_vertical(self, stem: VerticalOwnershipSpan) -> bool {
        self.left_x <= stem.x
            && stem.x <= self.right_x
            && stem.top <= self.lane_y
            && self.lane_y <= stem.bottom
    }

    fn owns_endpoint(self, node_index: usize, x: usize) -> bool {
        (self.left_stem.node_index == node_index && self.left_x == x)
            || (self.right_stem.node_index == node_index && self.right_x == x)
    }
}

fn vertical_stems_conflict(
    left: VerticalOwnershipSpan,
    right: VerticalOwnershipSpan,
    compatible_shared_endpoints: CompatibleSharedEndpoints,
) -> bool {
    stems_overlap(left, right)
        && !(compatible_shared_endpoints.contains(left.node_index)
            && left.node_index == right.node_index)
}

fn stems_overlap(left: VerticalOwnershipSpan, right: VerticalOwnershipSpan) -> bool {
    left.x == right.x && left.top.max(right.top) <= left.bottom.min(right.bottom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compatible_shared_endpoint_stems_can_merge() {
        let short = geometry(0, 1, 2, 10, 8, 2);
        let long = geometry(0, 2, 4, 20, 8, 4);

        assert!(!short.has_owner_collision_with(long, CompatibleSharedEndpoints::single(0)));
        assert!(short.has_owner_collision_with(long, CompatibleSharedEndpoints::default()));
    }

    #[test]
    fn remote_stem_crossing_is_not_exempted_by_a_shared_endpoint() {
        let short = geometry(0, 1, 2, 10, 8, 2);
        let long = geometry(0, 2, 4, 20, 8, 4);
        let short_with_remote_stem_inside_long_lane = HorizontalEdgeGeometry {
            right_stem: VerticalOwnershipSpan {
                node_index: 1,
                x: 10,
                top: 2,
                bottom: 8,
            },
            ..short
        };

        assert!(
            short_with_remote_stem_inside_long_lane
                .has_owner_collision_with(long, CompatibleSharedEndpoints::single(0),)
        );
    }

    #[test]
    fn both_shared_endpoints_can_be_exempted_independently() {
        let first = geometry(0, 1, 2, 10, 8, 8);
        let second = geometry(0, 1, 4, 10, 8, 8);

        assert!(
            !first.has_owner_collision_with(second, CompatibleSharedEndpoints::single(0).add(1),)
        );
        assert!(first.has_owner_collision_with(second, CompatibleSharedEndpoints::single(0),));
    }

    fn geometry(
        left_node: usize,
        right_node: usize,
        lane_y: usize,
        right_x: usize,
        left_port_y: usize,
        right_port_y: usize,
    ) -> HorizontalEdgeGeometry {
        HorizontalEdgeGeometry {
            lane_y,
            left_x: 0,
            right_x,
            left_port_y,
            right_port_y,
            left_stem: VerticalOwnershipSpan {
                node_index: left_node,
                x: 0,
                top: lane_y.min(left_port_y),
                bottom: lane_y.max(left_port_y),
            },
            right_stem: VerticalOwnershipSpan {
                node_index: right_node,
                x: right_x,
                top: lane_y.min(right_port_y),
                bottom: lane_y.max(right_port_y),
            },
        }
    }
}
