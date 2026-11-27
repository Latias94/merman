//! Inline coordinate evidence from the final Flowchart and Agentflow DB owners.

use merman_core_preprocess::grid_coordinate::{
    GridCoordinateError, GridCoordinateField, GridCoordinateValue, validate_grid_coordinate,
};
use merman_core_preprocess::yaml::InlineGridNonfinite;
use merman_model::grid::{
    GridCoordinate, GridHorizontalAlign, GridInlinePlacement, GridInlinePlacements,
    GridPlacementOwner, GridPlacementSpec, GridVerticalAlign,
};
use merman_model::{OperationControl, OperationControlResult, SourceSpan};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default)]
pub struct InlineGridCoordinates(Vec<InlineGridPlacement>);

#[derive(Debug, Clone, Copy)]
pub struct InlineGridCoordinate<'a> {
    pub id: &'a str,
    pub field: GridCoordinateField,
    pub value: GridCoordinateValue<'a>,
    /// The parser-relative metadata token that last wrote this field.
    pub span: SourceSpan,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn grid_presentation_alignment_does_not_advance_coordinate_cancellation() {
        let align = json!({"horizontalAlign": "left"});
        let invalid = json!({"row": false});
        let span = SourceSpan::new(50, 64);
        for capture in [false, true] {
            let mut replay = InlineGridReplay::new(capture);
            replay.vertices.update(
                0,
                align.as_object().unwrap(),
                Some(span),
                InlineGridNonfinite::default(),
            );
            replay.vertices.update(
                256,
                invalid.as_object().unwrap(),
                Some(span),
                InlineGridNonfinite::default(),
            );

            let finish = OperationControl::new();
            finish.cancel_after_checkpoints(1);
            let coordinates = replay
                .finish(
                    |slot| {
                        assert!(capture || slot == 256);
                        Ok((
                            if slot == 0 { "A" } else { "B" },
                            Some(GridPlacementOwner::Vertex(slot)),
                        ))
                    },
                    |_| unreachable!("only vertices were recorded"),
                    &finish,
                )
                .unwrap();

            let validate = OperationControl::new();
            validate.cancel_after_checkpoints(1);
            let (error, actual_span) = if capture {
                coordinates
                    .validate_placements_controlled(&validate)
                    .unwrap()
                    .unwrap_err()
            } else {
                coordinates
                    .validate_controlled(&validate)
                    .unwrap()
                    .unwrap_err()
            };
            assert_eq!(error.placement_id, "B");
            assert_eq!(actual_span, span);
        }
    }
}

impl InlineGridCoordinates {
    /// Visits vertices and then subgraphs in DB order, with row before column on each item.
    pub fn iter(&self) -> impl Iterator<Item = InlineGridCoordinate<'_>> {
        self.0.iter().flat_map(|placement| {
            placement
                .fields
                .coordinates()
                .map(move |(field, assignment)| InlineGridCoordinate {
                    id: &placement.id,
                    field,
                    value: assignment.value.as_value(),
                    span: assignment.span,
                })
        })
    }

    /// Validates the successful replay after the caller has admitted effective Grid placements.
    pub fn validate_controlled(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<(), (GridCoordinateError<'_>, SourceSpan)>> {
        for coordinate in self.iter() {
            control.checkpoint()?;
            if let Err(error) = validate_grid_coordinate(
                coordinate.id,
                None,
                coordinate.field,
                Some(coordinate.value),
            ) {
                return Ok(Err((error, coordinate.span)));
            }
        }
        control.checkpoint()?;
        Ok(Ok(()))
    }

    /// Validates DB coordinates and freezes the fields with an owner in the render model.
    /// Call after admitting effective configured placements.
    pub fn validate_placements_controlled(
        &self,
        control: &OperationControl,
    ) -> OperationControlResult<Result<GridInlinePlacements, (GridCoordinateError<'_>, SourceSpan)>>
    {
        if let Err(error) = self.validate_controlled(control)? {
            return Ok(Err(error));
        }
        let mut entries = Vec::new();
        for placement in &self.0 {
            control.checkpoint()?;
            let Some(owner) = placement.owner else {
                continue;
            };
            let fields = GridPlacementSpec {
                row: placement
                    .fields
                    .row
                    .as_ref()
                    .map(GridAssignment::coordinate),
                column: placement
                    .fields
                    .column
                    .as_ref()
                    .map(GridAssignment::coordinate),
                horizontal_align: placement.fields.horizontal_align,
                vertical_align: placement.fields.vertical_align,
            };
            if fields != GridPlacementSpec::default() {
                entries.push(GridInlinePlacement {
                    owner,
                    id: placement.id.clone(),
                    placement: fields,
                });
            }
        }
        control.checkpoint()?;
        Ok(Ok(GridInlinePlacements::from_entries(entries)))
    }
}

#[derive(Debug, Clone)]
struct InlineGridPlacement {
    id: String,
    owner: Option<GridPlacementOwner>,
    fields: InlineGridFields,
}

#[derive(Debug, Clone, Default)]
struct InlineGridFields {
    row: Option<GridAssignment>,
    column: Option<GridAssignment>,
    horizontal_align: Option<GridHorizontalAlign>,
    vertical_align: Option<GridVerticalAlign>,
}

impl InlineGridFields {
    fn coordinates(&self) -> impl Iterator<Item = (GridCoordinateField, &GridAssignment)> {
        [
            (GridCoordinateField::Row, self.row.as_ref()),
            (GridCoordinateField::Column, self.column.as_ref()),
        ]
        .into_iter()
        .filter_map(|(field, assignment)| Some((field, assignment?)))
    }
}

#[derive(Debug, Clone)]
struct GridAssignment {
    value: RetainedCoordinateValue,
    span: SourceSpan,
}

impl GridAssignment {
    fn coordinate(&self) -> GridCoordinate {
        let GridCoordinateValue::Number(number) = self.value.as_value() else {
            unreachable!("a validated grid coordinate is a number");
        };
        GridCoordinate::new(number as i64).expect("validated grid coordinate is a safe integer")
    }
}

#[derive(Debug, Clone)]
enum RetainedCoordinateValue {
    Scalar(Value),
    Nonfinite(f64),
    Array,
    Object,
}

impl RetainedCoordinateValue {
    fn from_metadata(value: &Value) -> Self {
        match value {
            Value::Array(_) => Self::Array,
            Value::Object(_) => Self::Object,
            scalar => Self::Scalar(scalar.clone()),
        }
    }

    fn as_value(&self) -> GridCoordinateValue<'_> {
        match self {
            Self::Scalar(value) => value.into(),
            Self::Nonfinite(value) => GridCoordinateValue::Number(*value),
            Self::Array => GridCoordinateValue::Array,
            Self::Object => GridCoordinateValue::Object,
        }
    }
}

/// Sparse assignments indexed by the existing DB slots, independent of metadata write order.
pub(crate) struct InlineGridReplay {
    pub(crate) vertices: GridMetadata,
    pub(crate) subgraphs: GridMetadata,
}

impl InlineGridReplay {
    pub(crate) fn new(capture_placements: bool) -> Self {
        Self {
            vertices: GridMetadata::new(capture_placements),
            subgraphs: GridMetadata::new(capture_placements),
        }
    }

    pub(crate) fn finish<'a>(
        self,
        mut vertex: impl FnMut(usize) -> OperationControlResult<(&'a str, Option<GridPlacementOwner>)>,
        mut subgraph: impl FnMut(usize) -> (&'a str, Option<GridPlacementOwner>),
        control: &OperationControl,
    ) -> OperationControlResult<InlineGridCoordinates> {
        let entries = self
            .vertices
            .entries
            .into_iter()
            .map(|(slot, fields)| (true, slot, fields))
            .chain(
                self.subgraphs
                    .entries
                    .into_iter()
                    .map(|(slot, fields)| (false, slot, fields)),
            );
        let mut placements = Vec::new();
        for (index, (is_vertex, slot, fields)) in entries.enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            let (id, owner) = if is_vertex {
                vertex(slot)?
            } else {
                subgraph(slot)
            };
            placements.push(InlineGridPlacement {
                id: id.to_owned(),
                owner,
                fields,
            });
        }
        Ok(InlineGridCoordinates(placements))
    }
}

pub(crate) struct GridMetadata {
    entries: BTreeMap<usize, InlineGridFields>,
    capture_alignments: bool,
}

impl GridMetadata {
    fn new(capture_alignments: bool) -> Self {
        Self {
            entries: BTreeMap::new(),
            capture_alignments,
        }
    }

    pub(crate) fn update(
        &mut self,
        slot: usize,
        metadata: &Map<String, Value>,
        span: Option<SourceSpan>,
        nonfinite: InlineGridNonfinite,
    ) {
        let Some(span) = span else { return };
        let row = metadata.get("row");
        let column = metadata.get("column");
        let horizontal_align = self
            .capture_alignments
            .then(|| metadata.get("horizontalAlign"))
            .flatten();
        let vertical_align = self
            .capture_alignments
            .then(|| metadata.get("verticalAlign"))
            .flatten();
        if [row, column, horizontal_align, vertical_align]
            .iter()
            .all(Option::is_none)
        {
            return;
        }
        let fields = self.entries.entry(slot).or_default();
        for (field, target, value) in [
            (GridCoordinateField::Row, &mut fields.row, row),
            (GridCoordinateField::Column, &mut fields.column, column),
        ] {
            if let Some(value) = value {
                let value = nonfinite.get(field).map_or_else(
                    || RetainedCoordinateValue::from_metadata(value),
                    RetainedCoordinateValue::Nonfinite,
                );
                *target = Some(GridAssignment { value, span });
            }
        }
        if let Some(value) = horizontal_align {
            fields.horizontal_align = value.as_str().and_then(GridHorizontalAlign::from_keyword);
        }
        if let Some(value) = vertical_align {
            fields.vertical_align = value.as_str().and_then(GridVerticalAlign::from_keyword);
        }
    }

    #[cfg(feature = "diagram-agentflow")]
    pub(crate) fn clear(&mut self, slot: usize) {
        self.entries.remove(&slot);
    }
}
