//! Mermaid 12.4 placement for the direct children of one grid container.

use crate::resources::{OperationWorkError, OperationWorkMeter};
use merman_model::OperationPhase;
pub(super) use merman_model::grid::{
    GridCoordinate, GridHorizontalAlign, GridPlacementSpec, GridVerticalAlign,
};
use std::cmp::Ordering;
use std::collections::{HashMap, TryReserveError};
use std::fmt;

#[derive(Debug, Clone, Copy)]
pub struct GridItem<'a> {
    pub id: &'a str,
    pub authored_id: Option<&'a str>,
    /// Ordinal from the complete measured graph, before partitioning by container.
    pub source_ordinal: Option<usize>,
    pub inline: GridPlacementSpec,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GridPlacementOptions {
    /// Positive row-major width, or zero to use the ceiling of the square root of child count.
    pub columns: u64,
    pub horizontal_align: GridHorizontalAlign,
    pub vertical_align: GridVerticalAlign,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridCell {
    pub row: GridCoordinate,
    pub column: GridCoordinate,
}

impl fmt::Display for GridCell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {})", self.row.get(), self.column.get())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridResolvedPlacement {
    /// Index in the caller's input slice.
    pub item_index: usize,
    pub cell: GridCell,
    pub horizontal_align: GridHorizontalAlign,
    pub vertical_align: GridVerticalAlign,
    pub explicit_row: bool,
    pub explicit_column: bool,
}

impl GridResolvedPlacement {
    pub const fn explicit_cell(self) -> bool {
        self.explicit_row && self.explicit_column
    }
}

#[derive(Debug)]
pub struct GridCellStack {
    pub cell: GridCell,
    pub vertical_align: GridVerticalAlign,
    first_placement: usize,
    additional_placements: Vec<usize>,
}

impl GridCellStack {
    /// Indices in `GridPlacementResult::placements`, in stack order.
    pub fn placement_indices(&self) -> impl Iterator<Item = usize> + '_ {
        std::iter::once(self.first_placement).chain(self.additional_placements.iter().copied())
    }
}

#[derive(Debug)]
pub struct GridPlacementResult {
    /// Source order, with UTF-16 emitted-ID order breaking ordinal ties.
    pub placements: Vec<GridResolvedPlacement>,
    /// Occupied cells in Mermaid's insertion order. Stacks have at least one member.
    pub cells: Vec<GridCellStack>,
}

#[derive(Debug, thiserror::Error)]
pub enum GridPlacementError<'a> {
    #[error(transparent)]
    Work(#[from] OperationWorkError),
    #[error("layout backend `grid` could not allocate layout storage")]
    AllocationFailed(#[from] TryReserveError),
    #[error("GRID_CELL_ALIGNMENT_CONFLICT: Conflicting vertical alignment in cell {cell}")]
    AlignmentConflict {
        container_id: &'a str,
        cell: GridCell,
        /// Emitted IDs and effective vertical alignments through the first conflicting item.
        occupants: Vec<(&'a str, GridVerticalAlign)>,
    },
}

/// Resolves one container's measured direct children. Occurrence slots start at zero per call.
///
/// The caller supplies validated placement fields and excludes synthetic edge-label nodes.
/// The result is published only after the last layout checkpoint succeeds.
pub fn resolve_grid_placements<'a>(
    container_id: Option<&'a str>,
    items: &[GridItem<'a>],
    occurrences: &HashMap<&str, &[GridPlacementSpec]>,
    options: GridPlacementOptions,
    work: &OperationWorkMeter,
) -> Result<GridPlacementResult, GridPlacementError<'a>> {
    work.charge(items.len())?;
    let sorted_items = source_order(items, work)?;
    let mut placements = Vec::new();
    placements.try_reserve_exact(items.len())?;
    let mut next_occurrence_by_id = HashMap::new();
    next_occurrence_by_id.try_reserve(items.len().min(occurrences.len()))?;

    for item_index in sorted_items {
        let item = items[item_index];
        let id = item.authored_id.unwrap_or(item.id);
        // The occurrence table and its counter both hash the authored identity.
        let slots = if occurrences.is_empty() {
            work.charge(1)?;
            None
        } else {
            let id_work = id
                .len()
                .checked_mul(2)
                .and_then(|bytes| bytes.checked_add(1))
                .ok_or_else(|| work.arithmetic_overflow())?;
            work.charge(id_work)?;
            occurrences.get(id)
        };
        let configured = if let Some(slots) = slots {
            let next = next_occurrence_by_id.entry(id).or_insert(0usize);
            let spec = slots.get(*next).copied().unwrap_or_default();
            // At most one increment per input item.
            *next += 1;
            spec
        } else {
            GridPlacementSpec::default()
        };

        let row = item.inline.row.or(configured.row);
        let column = item.inline.column.or(configured.column);
        placements.push(GridResolvedPlacement {
            item_index,
            cell: GridCell {
                row: row.unwrap_or_default(),
                column: column.unwrap_or_default(),
            },
            horizontal_align: item
                .inline
                .horizontal_align
                .or(configured.horizontal_align)
                .unwrap_or(options.horizontal_align),
            vertical_align: item
                .inline
                .vertical_align
                .or(configured.vertical_align)
                .unwrap_or(options.vertical_align),
            explicit_row: row.is_some(),
            explicit_column: column.is_some(),
        });
    }

    let mut cells = SparseCells::default();
    for (index, placement) in placements.iter().enumerate() {
        work.charge(1)?;
        if !placement.explicit_cell() {
            continue;
        }
        let Some(&stack_index) = cells.index.get(&placement.cell) else {
            cells.push(index, placement)?;
            continue;
        };
        let stack = &mut cells.stacks[stack_index];
        if stack.vertical_align != placement.vertical_align {
            // The existing stack and the current item are disjoint members of `placements`.
            let count = stack.additional_placements.len() + 2;
            work.charge(count)?;
            let mut occupants = Vec::new();
            occupants.try_reserve_exact(count)?;
            for member in stack.placement_indices().chain(std::iter::once(index)) {
                work.checkpoint(OperationPhase::Layout)?;
                let member = placements[member];
                occupants.push((items[member.item_index].id, member.vertical_align));
            }
            work.checkpoint(OperationPhase::Layout)?;
            return Err(GridPlacementError::AlignmentConflict {
                container_id: container_id.unwrap_or("root"),
                cell: placement.cell,
                occupants,
            });
        }
        stack.additional_placements.try_reserve(1)?;
        stack.additional_placements.push(index);
    }

    let columns = if options.columns == 0 {
        items.len().saturating_sub(1).isqrt() as u64 + 1
    } else {
        options.columns
    };
    let mut next_auto = 0;
    let mut next_column_by_row = HashMap::new();
    let mut next_row_by_column = HashMap::new();
    for (index, placement) in placements.iter_mut().enumerate() {
        work.charge(1)?;
        if placement.explicit_cell() {
            continue;
        }
        let GridCell { row, column } = placement.cell;
        placement.cell = if placement.explicit_row {
            next_column_by_row.try_reserve(1)?;
            let next = next_column_by_row.entry(row).or_default();
            cells.next_free(work, next, |index| {
                Some(GridCell {
                    row,
                    column: GridCoordinate::new((index + 1) as i64)?,
                })
            })?
        } else if placement.explicit_column {
            next_row_by_column.try_reserve(1)?;
            let next = next_row_by_column.entry(column).or_default();
            cells.next_free(work, next, |index| {
                Some(GridCell {
                    row: GridCoordinate::new((index + 1) as i64)?,
                    column,
                })
            })?
        } else {
            cells.next_free(work, &mut next_auto, |index| {
                Some(GridCell {
                    row: GridCoordinate::new((index / columns + 1) as i64)?,
                    column: GridCoordinate::new((index % columns + 1) as i64)?,
                })
            })?
        };
        cells.push(index, placement)?;
    }
    work.checkpoint(OperationPhase::Layout)?;
    Ok(GridPlacementResult {
        placements,
        cells: cells.stacks,
    })
}

#[derive(Default)]
struct SparseCells {
    index: HashMap<GridCell, usize>,
    stacks: Vec<GridCellStack>,
}

impl SparseCells {
    fn push(
        &mut self,
        index: usize,
        placement: &GridResolvedPlacement,
    ) -> Result<(), TryReserveError> {
        self.index.try_reserve(1)?;
        self.stacks.try_reserve(1)?;
        let previous = self.index.insert(placement.cell, self.stacks.len());
        debug_assert!(previous.is_none());
        self.stacks.push(GridCellStack {
            cell: placement.cell,
            vertical_align: placement.vertical_align,
            first_placement: index,
            additional_placements: Vec::new(),
        });
        Ok(())
    }

    fn next_free(
        &self,
        work: &OperationWorkMeter,
        cursor: &mut u64,
        cell_at: impl Fn(u64) -> Option<GridCell>,
    ) -> Result<GridCell, OperationWorkError> {
        loop {
            work.charge(1)?;
            let index = *cursor;
            // Every computed track is one-based and at most index + 1.
            if index >= GridCoordinate::MAX.get() as u64 {
                return Err(work.arithmetic_overflow());
            }
            *cursor = index + 1;
            let cell = cell_at(index).ok_or_else(|| work.arithmetic_overflow())?;
            if !self.index.contains_key(&cell) {
                return Ok(cell);
            }
        }
    }
}

fn source_order<'a>(
    items: &[GridItem<'a>],
    work: &OperationWorkMeter,
) -> Result<Vec<usize>, GridPlacementError<'a>> {
    let mut order = Vec::new();
    order.try_reserve_exact(items.len())?;
    let mut ordered = true;
    for index in 0..items.len() {
        work.checkpoint(OperationPhase::Layout)?;
        order.push(index);
        if ordered && index > 0 {
            ordered = compare(items, index - 1, index, work)?.is_le();
        }
    }
    if ordered {
        return Ok(order);
    }

    // Sort in place with fallible comparisons. The input index preserves stable ties.
    for parent in (0..order.len() / 2).rev() {
        sift_down(items, &mut order, parent, work)?;
    }
    for end in (1..order.len()).rev() {
        work.charge(1)?;
        order.swap(0, end);
        sift_down(items, &mut order[..end], 0, work)?;
    }
    Ok(order)
}

fn sift_down(
    items: &[GridItem<'_>],
    order: &mut [usize],
    mut root: usize,
    work: &OperationWorkMeter,
) -> Result<(), OperationWorkError> {
    while root < order.len() / 2 {
        let mut child = root * 2 + 1;
        if child + 1 < order.len() && compare(items, order[child], order[child + 1], work)?.is_lt()
        {
            child += 1;
        }
        if compare(items, order[root], order[child], work)?.is_ge() {
            break;
        }
        order.swap(root, child);
        root = child;
    }
    Ok(())
}

fn compare(
    items: &[GridItem<'_>],
    left: usize,
    right: usize,
    work: &OperationWorkMeter,
) -> Result<Ordering, OperationWorkError> {
    work.charge(1)?;
    let a = items[left];
    let b = items[right];
    let ordinal = a
        .source_ordinal
        .unwrap_or(usize::MAX)
        .cmp(&b.source_ordinal.unwrap_or(usize::MAX));
    if !ordinal.is_eq() {
        return Ok(ordinal);
    }
    let mut a_units = a.id.encode_utf16();
    let mut b_units = b.id.encode_utf16();
    loop {
        work.charge(1)?;
        match (a_units.next(), b_units.next()) {
            (Some(a), Some(b)) if a == b => continue,
            (None, None) => return Ok(left.cmp(&right)),
            (a, b) => return Ok(a.cmp(&b)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resources::{RenderResourcePolicy, ResourceLimitId};
    use merman_model::OperationControl;

    fn at(row: Option<i64>, column: Option<i64>) -> GridPlacementSpec {
        GridPlacementSpec {
            row: row.map(|value| GridCoordinate::new(value).unwrap()),
            column: column.map(|value| GridCoordinate::new(value).unwrap()),
            ..GridPlacementSpec::default()
        }
    }

    fn item(id: &str, source_ordinal: usize, inline: GridPlacementSpec) -> GridItem<'_> {
        GridItem {
            id,
            authored_id: None,
            source_ordinal: Some(source_ordinal),
            inline,
        }
    }

    fn work() -> OperationWorkMeter {
        OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input())
    }

    fn coordinates(result: &GridPlacementResult) -> Vec<(usize, i64, i64)> {
        result
            .placements
            .iter()
            .map(|p| (p.item_index, p.cell.row.get(), p.cell.column.get()))
            .collect()
    }

    #[test]
    fn reserves_later_explicit_cells_before_auto() {
        let items = [
            item("auto", 0, at(None, None)),
            item("explicit", 1, at(Some(1), Some(1))),
        ];
        let result = resolve_grid_placements(
            None,
            &items,
            &HashMap::new(),
            GridPlacementOptions::default(),
            &work(),
        )
        .unwrap();
        assert_eq!(coordinates(&result), [(0, 1, 2), (1, 1, 1)]);
        assert_eq!(
            result
                .cells
                .iter()
                .flat_map(|stack| stack.placement_indices())
                .collect::<Vec<_>>(),
            [1, 0]
        );
    }

    #[test]
    fn matches_upstream_mixed_placements_and_stacks() {
        use GridHorizontalAlign::{Left, Right};
        use GridVerticalAlign::Bottom;

        let items = [
            item("explicit-a", 0, at(Some(1), Some(1))),
            item("explicit-b", 1, at(Some(100), Some(3))),
            item(
                "stack-top",
                2,
                GridPlacementSpec {
                    horizontal_align: Some(Left),
                    vertical_align: Some(Bottom),
                    ..at(Some(2), Some(2))
                },
            ),
            item(
                "stack-bottom",
                3,
                GridPlacementSpec {
                    horizontal_align: Some(Right),
                    vertical_align: Some(Bottom),
                    ..at(Some(2), Some(2))
                },
            ),
            item("row-only", 4, at(Some(1), None)),
            item("column-only", 5, at(None, Some(1))),
            item("auto-a", 6, at(None, None)),
            item("auto-b", 7, at(None, None)),
            item("map-only", 8, at(None, None)),
            item(
                "metadata-wins",
                9,
                GridPlacementSpec {
                    horizontal_align: Some(Left),
                    ..at(None, Some(4))
                },
            ),
        ];
        let map_only = [at(Some(3), Some(3))];
        let metadata_wins = [GridPlacementSpec {
            horizontal_align: Some(Right),
            ..at(Some(4), Some(2))
        }];
        let occurrences = HashMap::from([
            ("map-only", map_only.as_slice()),
            ("metadata-wins", metadata_wins.as_slice()),
        ]);
        let options = GridPlacementOptions {
            columns: 3,
            ..GridPlacementOptions::default()
        };
        let result = resolve_grid_placements(None, &items, &occurrences, options, &work()).unwrap();
        assert_eq!(
            coordinates(&result),
            [
                (0, 1, 1),
                (1, 100, 3),
                (2, 2, 2),
                (3, 2, 2),
                (4, 1, 2),
                (5, 2, 1),
                (6, 1, 3),
                (7, 2, 3),
                (8, 3, 3),
                (9, 4, 4),
            ]
        );
        let stack = &result.cells[2];
        assert_eq!((stack.cell.row.get(), stack.cell.column.get()), (2, 2));
        assert_eq!(stack.placement_indices().collect::<Vec<_>>(), [2, 3]);
        assert_eq!(stack.vertical_align, Bottom);
        assert_eq!(result.placements[3].horizontal_align, Right);
        assert_eq!(result.placements[9].horizontal_align, Left);
    }

    #[test]
    fn advances_partial_cursors_past_other_placement_modes() {
        let items = [
            item("explicit", 0, at(Some(1), Some(1))),
            item("row-a", 1, at(Some(1), None)),
            item("column-a", 2, at(None, Some(2))),
            item("auto-a", 3, at(None, None)),
            item("row-b", 4, at(Some(1), None)),
            item("column-b", 5, at(None, Some(2))),
            item("auto-b", 6, at(None, None)),
        ];
        let result = resolve_grid_placements(
            None,
            &items,
            &HashMap::new(),
            GridPlacementOptions {
                columns: 3,
                ..GridPlacementOptions::default()
            },
            &work(),
        )
        .unwrap();
        assert_eq!(
            coordinates(&result),
            [
                (0, 1, 1),
                (1, 1, 2),
                (2, 2, 2),
                (3, 1, 3),
                (4, 1, 4),
                (5, 3, 2),
                (6, 2, 1)
            ]
        );
    }

    #[test]
    fn handles_signed_zero_and_safe_integer_extremes_as_sparse_tracks() {
        const MAX: i64 = 9_007_199_254_740_991;
        let items = [
            item("stack-a", 0, at(Some(-MAX), Some(0))),
            item("stack-b", 1, at(Some(-MAX), Some(0))),
            item("row-only", 2, at(Some(-MAX), None)),
            item("column-only", 3, at(None, Some(MAX))),
            item("auto", 4, at(None, None)),
            item("metadata-wins", 5, at(None, Some(0))),
            item("auto-next", 6, at(None, None)),
        ];
        let zero_override = [at(Some(0), Some(-3))];
        let occurrences = HashMap::from([("metadata-wins", zero_override.as_slice())]);
        let options = GridPlacementOptions {
            columns: u64::MAX,
            ..GridPlacementOptions::default()
        };
        let result = resolve_grid_placements(None, &items, &occurrences, options, &work()).unwrap();
        assert_eq!(
            coordinates(&result),
            [
                (0, -MAX, 0),
                (1, -MAX, 0),
                (2, -MAX, 1),
                (3, 1, MAX),
                (4, 1, 1),
                (5, 0, 0),
                (6, 1, 2)
            ]
        );
        assert_eq!(result.cells.len(), 6);
        assert!(result.placements[5].explicit_cell());
        assert!(GridCoordinate::new(MAX + 1).is_none());
        assert!(GridCoordinate::new(-MAX - 1).is_none());
    }

    #[test]
    fn consumes_authored_id_occurrences_in_source_order_and_resets_per_container() {
        let items = [
            GridItem {
                authored_id: Some("service"),
                ..item("rendered-2", 1, at(None, None))
            },
            GridItem {
                authored_id: Some("service"),
                ..item("rendered-1", 0, at(None, None))
            },
            GridItem {
                authored_id: Some("service"),
                ..item("rendered-3", 2, at(None, None))
            },
        ];
        let slots = [at(Some(1), Some(1)), at(Some(1), Some(2))];
        let generated_slot = [at(Some(99), Some(99))];
        let occurrences = HashMap::from([
            ("service", slots.as_slice()),
            ("rendered-1", generated_slot.as_slice()),
        ]);
        let work = work();
        let options = GridPlacementOptions::default();
        let result =
            resolve_grid_placements(Some("left"), &items, &occurrences, options, &work).unwrap();
        assert_eq!(coordinates(&result), [(1, 1, 1), (0, 1, 2), (2, 2, 1)]);
        assert!(!result.placements[2].explicit_row && !result.placements[2].explicit_column);
        let result =
            resolve_grid_placements(Some("right"), &items[..1], &occurrences, options, &work)
                .unwrap();
        assert_eq!(coordinates(&result), [(0, 1, 1)]);
    }

    #[test]
    fn compares_emitted_ids_by_utf16_for_tied_or_missing_ordinals() {
        let items = [
            GridItem {
                source_ordinal: None,
                ..item("b", 0, at(None, None))
            },
            item("\u{e000}", 1, at(None, None)),
            GridItem {
                source_ordinal: None,
                ..item("a", 0, at(None, None))
            },
            item("\u{10000}", 1, at(None, None)),
        ];
        let result = resolve_grid_placements(
            None,
            &items,
            &HashMap::new(),
            GridPlacementOptions::default(),
            &work(),
        )
        .unwrap();
        assert_eq!(
            coordinates(&result),
            [(3, 1, 1), (1, 1, 2), (2, 2, 1), (0, 2, 2)]
        );
    }

    #[test]
    fn reports_conflicting_stack_members_in_source_order() {
        use GridVerticalAlign::{Bottom, Top};

        let top = GridPlacementSpec {
            vertical_align: Some(Top),
            ..at(Some(1), Some(1))
        };
        let bottom = GridPlacementSpec {
            vertical_align: Some(Bottom),
            ..top
        };
        let items = [
            item("three", 2, bottom),
            item("two", 1, top),
            item("one", 0, top),
        ];
        let error = resolve_grid_placements(
            Some("group"),
            &items,
            &HashMap::new(),
            GridPlacementOptions::default(),
            &work(),
        )
        .unwrap_err();
        assert_eq!(
            error.to_string(),
            "GRID_CELL_ALIGNMENT_CONFLICT: Conflicting vertical alignment in cell (1, 1)"
        );
        let GridPlacementError::AlignmentConflict {
            container_id,
            cell,
            occupants,
        } = error
        else {
            panic!("expected a stack conflict");
        };
        assert_eq!(container_id, "group");
        assert_eq!((cell.row.get(), cell.column.get()), (1, 1));
        assert_eq!(occupants, [("one", Top), ("two", Top), ("three", Bottom)]);
    }

    #[test]
    fn uses_cumulative_ceiling_and_checks_cancellation_between_cell_probes() {
        let items = [
            item("auto", 0, at(None, None)),
            item("explicit", 1, at(Some(1), Some(1))),
        ];
        let place = |work: &OperationWorkMeter| {
            resolve_grid_placements(
                None,
                &items,
                &HashMap::new(),
                GridPlacementOptions::default(),
                work,
            )
        };
        let baseline = work();
        baseline.charge(5).unwrap();
        place(&baseline).unwrap();
        let exact = baseline.used();
        for ceiling in [exact, exact - 1] {
            let policy = RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxLayoutWorkUnits, ceiling)
                .unwrap();
            let meter = OperationWorkMeter::new(policy);
            meter.charge(5).unwrap();
            match place(&meter) {
                Ok(_) => assert_eq!(ceiling, exact),
                Err(GridPlacementError::Work(OperationWorkError::ResourceLimitExceeded(error))) => {
                    assert_eq!(ceiling, exact - 1);
                    assert_eq!(error.limit, ResourceLimitId::MaxLayoutWorkUnits.as_str());
                    assert_eq!((error.actual, error.max), (exact, ceiling));
                }
                other => panic!("unexpected placement result: {other:?}"),
            }
        }

        let control = OperationControl::new();
        // Permit the first (occupied) auto probe and cancel before the second probe.
        control.cancel_after_checkpoints(10);
        let meter = OperationWorkMeter::new_with_control(
            RenderResourcePolicy::unbounded_for_trusted_input(),
            control,
        );
        assert!(matches!(
            place(&meter),
            Err(GridPlacementError::Work(OperationWorkError::Cancelled(error)))
                if error.phase == OperationPhase::Layout
        ));
        assert_eq!(meter.used(), 9);
    }
}
