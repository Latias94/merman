use crate::{AsciiError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PhysicalPortSide {
    Top,
    Right,
    Bottom,
    Left,
}

impl PhysicalPortSide {
    pub(crate) const ALL: [Self; 4] = [Self::Top, Self::Right, Self::Bottom, Self::Left];

    pub(crate) const fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Right => Self::Left,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RelationPoint {
    x: usize,
    y: usize,
}

impl RelationPoint {
    pub(crate) const fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }

    pub(crate) const fn x(self) -> usize {
        self.x
    }

    pub(crate) const fn y(self) -> usize {
        self.y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RelationExtent {
    width: usize,
    height: usize,
}

impl RelationExtent {
    pub(crate) const fn new(width: usize, height: usize) -> Self {
        Self { width, height }
    }

    pub(crate) const fn width(self) -> usize {
        self.width
    }

    pub(crate) const fn height(self) -> usize {
        self.height
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RelationDirection {
    TopDown,
    BottomUp,
    LeftRight,
    RightLeft,
}

impl RelationDirection {
    pub(crate) fn try_from_model(
        raw: &str,
        diagram_type: &'static str,
        unsupported_feature: &'static str,
    ) -> Result<Self> {
        Self::parse(raw).ok_or(AsciiError::UnsupportedFeature {
            diagram_type,
            feature: unsupported_feature,
        })
    }

    pub(crate) const fn is_horizontal(self) -> bool {
        matches!(self, Self::LeftRight | Self::RightLeft)
    }

    pub(crate) const fn is_reversed(self) -> bool {
        matches!(self, Self::BottomUp | Self::RightLeft)
    }

    pub(crate) const fn transform(self) -> DirectionTransform {
        DirectionTransform { direction: self }
    }

    fn parse(raw: &str) -> Option<Self> {
        let raw = raw.trim();
        if raw.is_empty() || raw.eq_ignore_ascii_case("TB") || raw.eq_ignore_ascii_case("TD") {
            Some(Self::TopDown)
        } else if raw.eq_ignore_ascii_case("BT") {
            Some(Self::BottomUp)
        } else if raw.eq_ignore_ascii_case("LR") {
            Some(Self::LeftRight)
        } else if raw.eq_ignore_ascii_case("RL") {
            Some(Self::RightLeft)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DirectionTransform {
    direction: RelationDirection,
}

impl DirectionTransform {
    pub(crate) const fn is_horizontal(self) -> bool {
        self.direction.is_horizontal()
    }

    pub(crate) const fn is_reversed(self) -> bool {
        self.direction.is_reversed()
    }

    pub(crate) const fn reverses_vertical_order(self) -> bool {
        matches!(self.direction, RelationDirection::BottomUp)
    }

    pub(crate) fn require_horizontal(self) -> Result<Self> {
        if self.is_horizontal() {
            Ok(self)
        } else {
            Err(AsciiError::InvalidOption {
                field: "direction",
                message: "horizontal relation layout requires LR or RL",
            })
        }
    }

    pub(crate) fn order_stacked_boxes<T>(self, boxes: &mut [T]) -> Result<()> {
        match self.direction {
            RelationDirection::TopDown => Ok(()),
            RelationDirection::BottomUp => {
                boxes.reverse();
                Ok(())
            }
            RelationDirection::LeftRight | RelationDirection::RightLeft => {
                Err(AsciiError::InvalidOption {
                    field: "direction",
                    message: "stacked relation layout requires TB or BT",
                })
            }
        }
    }

    pub(crate) const fn map_extent(self, extent: RelationExtent) -> RelationExtent {
        match self.direction {
            RelationDirection::TopDown | RelationDirection::BottomUp => extent,
            RelationDirection::LeftRight | RelationDirection::RightLeft => {
                RelationExtent::new(extent.height, extent.width)
            }
        }
    }

    pub(crate) fn map_point(
        self,
        point: RelationPoint,
        canonical_extent: RelationExtent,
    ) -> Option<RelationPoint> {
        if point.x >= canonical_extent.width || point.y >= canonical_extent.height {
            return None;
        }
        match self.direction {
            RelationDirection::TopDown => Some(point),
            RelationDirection::BottomUp => Some(RelationPoint::new(
                point.x,
                canonical_extent
                    .height
                    .checked_sub(point.y.checked_add(1)?)?,
            )),
            RelationDirection::LeftRight => Some(RelationPoint::new(point.y, point.x)),
            RelationDirection::RightLeft => Some(RelationPoint::new(
                canonical_extent
                    .height
                    .checked_sub(point.y.checked_add(1)?)?,
                point.x,
            )),
        }
    }

    pub(crate) const fn map_port(self, side: PhysicalPortSide) -> PhysicalPortSide {
        match (self.direction, side) {
            (RelationDirection::TopDown, side) => side,
            (RelationDirection::BottomUp, PhysicalPortSide::Top) => PhysicalPortSide::Bottom,
            (RelationDirection::BottomUp, PhysicalPortSide::Right) => PhysicalPortSide::Right,
            (RelationDirection::BottomUp, PhysicalPortSide::Bottom) => PhysicalPortSide::Top,
            (RelationDirection::BottomUp, PhysicalPortSide::Left) => PhysicalPortSide::Left,
            (RelationDirection::LeftRight, PhysicalPortSide::Top) => PhysicalPortSide::Left,
            (RelationDirection::LeftRight, PhysicalPortSide::Right) => PhysicalPortSide::Bottom,
            (RelationDirection::LeftRight, PhysicalPortSide::Bottom) => PhysicalPortSide::Right,
            (RelationDirection::LeftRight, PhysicalPortSide::Left) => PhysicalPortSide::Top,
            (RelationDirection::RightLeft, PhysicalPortSide::Top) => PhysicalPortSide::Right,
            (RelationDirection::RightLeft, PhysicalPortSide::Right) => PhysicalPortSide::Bottom,
            (RelationDirection::RightLeft, PhysicalPortSide::Bottom) => PhysicalPortSide::Left,
            (RelationDirection::RightLeft, PhysicalPortSide::Left) => PhysicalPortSide::Top,
        }
    }

    /// Returns the physical top/bottom projection used by the existing vertical planner.
    ///
    /// The values remain authored source/target data; this method only chooses which value is
    /// painted at the physical top or bottom. Horizontal planners retain authored order and use
    /// [`Self::map_port`] for their physical endpoint sides.
    pub(crate) fn physical_vertical_pair<T>(self, source: T, target: T) -> (T, T) {
        match self.direction {
            RelationDirection::BottomUp => (target, source),
            RelationDirection::TopDown
            | RelationDirection::LeftRight
            | RelationDirection::RightLeft => (source, target),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_relation_directions() {
        for (raw, expected) in [
            ("", Some(RelationDirection::TopDown)),
            ("   ", Some(RelationDirection::TopDown)),
            ("TB", Some(RelationDirection::TopDown)),
            (" td ", Some(RelationDirection::TopDown)),
            ("Bt", Some(RelationDirection::BottomUp)),
            ("lR", Some(RelationDirection::LeftRight)),
            ("Rl", Some(RelationDirection::RightLeft)),
            ("horizontal", None),
        ] {
            assert_eq!(RelationDirection::parse(raw), expected, "raw={raw:?}");
        }
    }

    #[test]
    fn projects_relation_direction_helpers() {
        for (direction, horizontal, reversed) in [
            (RelationDirection::TopDown, false, false),
            (RelationDirection::BottomUp, false, true),
            (RelationDirection::LeftRight, true, false),
            (RelationDirection::RightLeft, true, true),
        ] {
            assert_eq!(direction.is_horizontal(), horizontal, "{direction:?}");
            assert_eq!(direction.is_reversed(), reversed, "{direction:?}");
        }
    }

    #[test]
    fn transforms_points_extents_and_ports_for_every_direction() {
        let canonical_extent = RelationExtent::new(5, 3);
        let canonical_point = RelationPoint::new(1, 0);
        for (direction, point, extent, ports) in [
            (
                RelationDirection::TopDown,
                RelationPoint::new(1, 0),
                RelationExtent::new(5, 3),
                [
                    PhysicalPortSide::Top,
                    PhysicalPortSide::Right,
                    PhysicalPortSide::Bottom,
                    PhysicalPortSide::Left,
                ],
            ),
            (
                RelationDirection::BottomUp,
                RelationPoint::new(1, 2),
                RelationExtent::new(5, 3),
                [
                    PhysicalPortSide::Bottom,
                    PhysicalPortSide::Right,
                    PhysicalPortSide::Top,
                    PhysicalPortSide::Left,
                ],
            ),
            (
                RelationDirection::LeftRight,
                RelationPoint::new(0, 1),
                RelationExtent::new(3, 5),
                [
                    PhysicalPortSide::Left,
                    PhysicalPortSide::Bottom,
                    PhysicalPortSide::Right,
                    PhysicalPortSide::Top,
                ],
            ),
            (
                RelationDirection::RightLeft,
                RelationPoint::new(2, 1),
                RelationExtent::new(3, 5),
                [
                    PhysicalPortSide::Right,
                    PhysicalPortSide::Bottom,
                    PhysicalPortSide::Left,
                    PhysicalPortSide::Top,
                ],
            ),
        ] {
            let transform = direction.transform();
            assert_eq!(
                transform.map_point(canonical_point, canonical_extent),
                Some(point),
                "{direction:?}",
            );
            assert_eq!(
                transform.map_extent(canonical_extent),
                extent,
                "{direction:?}",
            );
            for (canonical, expected) in PhysicalPortSide::ALL.into_iter().zip(ports) {
                assert_eq!(transform.map_port(canonical), expected, "{direction:?}");
            }
        }
        assert_eq!(
            RelationDirection::TopDown
                .transform()
                .map_point(RelationPoint::new(5, 0), canonical_extent),
            None,
        );
    }

    #[test]
    fn vertical_projection_does_not_mutate_authored_endpoint_order() {
        for (direction, expected) in [
            (RelationDirection::TopDown, ("source", "target")),
            (RelationDirection::BottomUp, ("target", "source")),
            (RelationDirection::LeftRight, ("source", "target")),
            (RelationDirection::RightLeft, ("source", "target")),
        ] {
            let authored = ("source", "target");
            assert_eq!(
                direction
                    .transform()
                    .physical_vertical_pair(authored.0, authored.1),
                expected,
            );
            assert_eq!(authored, ("source", "target"));
        }
    }

    #[test]
    fn horizontal_validation_rejects_vertical_transforms() {
        for direction in [RelationDirection::TopDown, RelationDirection::BottomUp] {
            assert!(matches!(
                direction.transform().require_horizontal(),
                Err(AsciiError::InvalidOption {
                    field: "direction",
                    message: "horizontal relation layout requires LR or RL",
                })
            ));
        }
        for direction in [RelationDirection::LeftRight, RelationDirection::RightLeft] {
            assert_eq!(
                direction.transform().require_horizontal(),
                Ok(direction.transform())
            );
        }
    }

    #[test]
    fn stacked_box_order_accepts_only_vertical_transforms() {
        let mut top_down = ["source", "target"];
        RelationDirection::TopDown
            .transform()
            .order_stacked_boxes(&mut top_down)
            .expect("top-down stacking should preserve order");
        assert_eq!(top_down, ["source", "target"]);

        let mut bottom_up = ["source", "target"];
        RelationDirection::BottomUp
            .transform()
            .order_stacked_boxes(&mut bottom_up)
            .expect("bottom-up stacking should reverse physical order");
        assert_eq!(bottom_up, ["target", "source"]);

        for direction in [RelationDirection::LeftRight, RelationDirection::RightLeft] {
            let mut boxes = ["source", "target"];
            assert!(matches!(
                direction.transform().order_stacked_boxes(&mut boxes),
                Err(AsciiError::InvalidOption {
                    field: "direction",
                    message: "stacked relation layout requires TB or BT",
                })
            ));
            assert_eq!(boxes, ["source", "target"]);
        }
    }

    #[test]
    fn preserves_family_specific_unsupported_direction_errors() {
        for (diagram_type, feature) in [
            ("class", "unknown class diagram directions"),
            ("er", "unknown ER diagram directions"),
        ] {
            let error = RelationDirection::try_from_model("sideways", diagram_type, feature)
                .expect_err("unknown directions should be rejected");
            assert!(matches!(
                error,
                AsciiError::UnsupportedFeature {
                    diagram_type: actual_diagram_type,
                    feature: actual_feature,
                } if actual_diagram_type == diagram_type && actual_feature == feature
            ));
        }
    }
}
