//! Inline coordinate evidence from the final Flowchart and Agentflow DB owners.

use merman_core_preprocess::grid_coordinate::{
    GridCoordinateError, GridCoordinateField, GridCoordinateValue, validate_grid_coordinate,
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

impl InlineGridCoordinates {
    /// Visits vertices and then subgraphs in DB order, with row before column on each item.
    pub fn iter(&self) -> impl Iterator<Item = InlineGridCoordinate<'_>> {
        self.0.iter().flat_map(|placement| {
            [
                (GridCoordinateField::Row, &placement.fields.row),
                (GridCoordinateField::Column, &placement.fields.column),
            ]
            .into_iter()
            .filter_map(|(field, assignment)| {
                assignment.as_ref().map(|assignment| InlineGridCoordinate {
                    id: &placement.id,
                    field,
                    value: assignment.value.as_value(),
                    span: assignment.span,
                })
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
}

#[derive(Debug, Clone)]
struct InlineGridPlacement {
    id: String,
    fields: InlineGridFields,
}

#[derive(Debug, Clone, Default)]
struct InlineGridFields {
    row: Option<GridAssignment>,
    column: Option<GridAssignment>,
}

#[derive(Debug, Clone)]
struct GridAssignment {
    value: RetainedCoordinateValue,
    span: SourceSpan,
}

#[derive(Debug, Clone)]
enum RetainedCoordinateValue {
    Scalar(Value),
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
            Self::Array => GridCoordinateValue::Array,
            Self::Object => GridCoordinateValue::Object,
        }
    }
}

/// Sparse assignments indexed by the existing DB slots, independent of metadata write order.
#[derive(Default)]
pub(crate) struct InlineGridReplay {
    pub(crate) vertices: GridMetadata,
    pub(crate) subgraphs: GridMetadata,
}

impl InlineGridReplay {
    pub(crate) fn finish<'a>(
        self,
        vertex_id: impl Fn(usize) -> &'a str,
        subgraph_id: impl Fn(usize) -> &'a str,
        control: &OperationControl,
    ) -> OperationControlResult<InlineGridCoordinates> {
        let entries = self
            .vertices
            .0
            .into_iter()
            .map(|(slot, fields)| (vertex_id(slot), fields))
            .chain(
                self.subgraphs
                    .0
                    .into_iter()
                    .map(|(slot, fields)| (subgraph_id(slot), fields)),
            );
        let mut placements = Vec::new();
        for (index, (id, fields)) in entries.enumerate() {
            if index.is_multiple_of(128) {
                control.checkpoint()?;
            }
            placements.push(InlineGridPlacement {
                id: id.to_owned(),
                fields,
            });
        }
        Ok(InlineGridCoordinates(placements))
    }
}

#[derive(Default)]
pub(crate) struct GridMetadata(BTreeMap<usize, InlineGridFields>);

impl GridMetadata {
    pub(crate) fn update(
        &mut self,
        slot: usize,
        metadata: &Map<String, Value>,
        span: Option<SourceSpan>,
    ) {
        let Some(span) = span else { return };
        let row = metadata.get("row");
        let column = metadata.get("column");
        if row.is_none() && column.is_none() {
            return;
        }
        let fields = self.0.entry(slot).or_default();
        for (target, value) in [(&mut fields.row, row), (&mut fields.column, column)] {
            if let Some(value) = value {
                *target = Some(GridAssignment {
                    value: RetainedCoordinateValue::from_metadata(value),
                    span,
                });
            }
        }
    }

    #[cfg(feature = "diagram-agentflow")]
    pub(crate) fn clear(&mut self, slot: usize) {
        self.0.remove(&slot);
    }
}
