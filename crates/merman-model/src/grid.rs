//! Typed Grid placement shared by presentation and layout.

use crate::{OperationControl, OperationControlResult};
use std::collections::HashSet;
use std::sync::Arc;

/// A signed Grid track label in JavaScript's safe-integer range.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridCoordinate(i64);

impl GridCoordinate {
    pub const MAX: Self = Self(9_007_199_254_740_991);

    pub const fn new(value: i64) -> Option<Self> {
        if value >= -Self::MAX.0 && value <= Self::MAX.0 {
            Some(Self(value))
        } else {
            None
        }
    }

    pub const fn get(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GridHorizontalAlign {
    Left,
    #[default]
    Center,
    Right,
}

impl GridHorizontalAlign {
    pub fn from_keyword(value: &str) -> Option<Self> {
        match value {
            "left" => Some(Self::Left),
            "center" => Some(Self::Center),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GridVerticalAlign {
    Top,
    #[default]
    Center,
    Bottom,
}

impl GridVerticalAlign {
    pub fn from_keyword(value: &str) -> Option<Self> {
        match value {
            "top" => Some(Self::Top),
            "center" => Some(Self::Center),
            "bottom" => Some(Self::Bottom),
            _ => None,
        }
    }
}

/// Validated optional fields of one inline placement or configured occurrence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GridPlacementSpec {
    pub row: Option<GridCoordinate>,
    pub column: Option<GridCoordinate>,
    pub horizontal_align: Option<GridHorizontalAlign>,
    pub vertical_align: Option<GridVerticalAlign>,
}

/// The slot in its own canonical family collection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GridPlacementOwner {
    Vertex(usize),
    Subgraph(usize),
    Connector(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridInlinePlacement {
    pub owner: GridPlacementOwner,
    pub id: String,
    pub placement: GridPlacementSpec,
}

/// Frozen sparse inline placements, shared with the family's layout adapter.
#[derive(Debug, Clone, Default)]
pub struct GridInlinePlacements(Option<Arc<[GridInlinePlacement]>>);

impl GridInlinePlacements {
    pub fn from_entries(entries: Vec<GridInlinePlacement>) -> Self {
        Self((!entries.is_empty()).then(|| entries.into()))
    }

    pub fn entries(&self) -> &[GridInlinePlacement] {
        self.0.as_deref().unwrap_or_default()
    }

    /// Accounts for the placement records and their retained authored IDs.
    pub fn retained_bytes(&self) -> usize {
        let entries = self.entries();
        entries
            .iter()
            .fold(std::mem::size_of_val(entries), |total, entry| {
                total.saturating_add(entry.id.len())
            })
    }

    /// Checks that every entry owns one distinct, matching family model slot.
    pub fn validate_owners_controlled<'a>(
        &self,
        id_for_owner: impl Fn(GridPlacementOwner) -> Option<&'a str>,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(), &'static str>> {
        let mut slots = HashSet::new();
        for entry in self.entries() {
            control.checkpoint()?;
            if id_for_owner(entry.owner) != Some(entry.id.as_str()) {
                return Ok(Err("grid placement owner does not match its model slot"));
            }
            if !slots.insert(entry.owner) {
                return Ok(Err("grid placement owners must be distinct"));
            }
        }
        Ok(Ok(()))
    }
}
