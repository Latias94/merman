use super::occupancy::SceneOccupancy;
use super::{PreparedRoute, RouteOwner};

#[derive(Clone, Copy)]
pub(super) struct RouteObstacles<'a> {
    pub(super) scene: &'a SceneOccupancy<'a>,
    pub(super) routes: &'a [PreparedRoute],
    pub(super) owner: &'a RouteOwner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum StepDirection {
    Up,
    Down,
    Left,
    Right,
}

impl StepDirection {
    pub(super) const fn opposite(self) -> Self {
        match self {
            Self::Up => Self::Down,
            Self::Down => Self::Up,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
        }
    }
}
