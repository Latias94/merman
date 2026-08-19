use super::horizontal::RelationGraphHorizontalDirection;
use crate::{AsciiError, Result};

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

    pub(crate) const fn horizontal_direction(self) -> RelationGraphHorizontalDirection {
        match self {
            Self::RightLeft => RelationGraphHorizontalDirection::RightLeft,
            Self::TopDown | Self::BottomUp | Self::LeftRight => {
                RelationGraphHorizontalDirection::LeftRight
            }
        }
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
        for (direction, horizontal, reversed, horizontal_direction) in [
            (
                RelationDirection::TopDown,
                false,
                false,
                RelationGraphHorizontalDirection::LeftRight,
            ),
            (
                RelationDirection::BottomUp,
                false,
                true,
                RelationGraphHorizontalDirection::LeftRight,
            ),
            (
                RelationDirection::LeftRight,
                true,
                false,
                RelationGraphHorizontalDirection::LeftRight,
            ),
            (
                RelationDirection::RightLeft,
                true,
                true,
                RelationGraphHorizontalDirection::RightLeft,
            ),
        ] {
            assert_eq!(direction.is_horizontal(), horizontal, "{direction:?}");
            assert_eq!(direction.is_reversed(), reversed, "{direction:?}");
            assert_eq!(
                direction.horizontal_direction(),
                horizontal_direction,
                "{direction:?}",
            );
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
