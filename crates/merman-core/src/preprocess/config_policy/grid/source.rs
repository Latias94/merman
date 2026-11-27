//! Tracks authored order and the winning coordinate source through Grid config merges.

use super::{GridCoordinateError, GridCoordinateField, js_array_index, ordered_entries};
use crate::preprocess::PreprocessedSource;
use crate::{
    MermaidConfig, OperationControl, OperationControlResult, ParseDiagnostic,
    ParseDiagnosticSpanKind, SourceSpan,
};
use merman_core_preprocess::yaml::GridPlacementsSource;
use merman_model::json::{ManagedJson, clone_value_nonrecursive_with_control};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, btree_map::Entry};
use std::ops::Range;

#[derive(Clone, Copy)]
struct Origin {
    span: SourceSpan,
    kind: ParseDiagnosticSpanKind,
}

struct FieldSource {
    field: GridCoordinateField,
    origin: Option<Origin>,
    nonfinite: Option<f64>,
}

#[derive(Default)]
struct PlacementSource {
    order: usize,
    fallback: Option<Origin>,
    slots: BTreeMap<Option<usize>, Vec<FieldSource>>,
}

#[derive(Default)]
pub(in crate::preprocess) struct GridSource {
    entries: BTreeMap<String, PlacementSource>,
    next_order: usize,
    fallback: Option<Origin>,
}

impl GridSource {
    pub(in crate::preprocess) fn capture(
        source: &PreprocessedSource,
        evidence: Option<GridPlacementsSource>,
        offset: usize,
        construct: SourceSpan,
        control: &OperationControl,
    ) -> OperationControlResult<Self> {
        let Some(evidence) = evidence else {
            return Ok(Self::default());
        };
        source.prepare_span_mapping_controlled(control)?;
        let map_key = |span: Range<usize>, kind| {
            let span = SourceSpan::new(offset + span.start, offset + span.end);
            source.try_map_span(span).map(|span| Origin { span, kind })
        };
        let fallback_kind = ParseDiagnosticSpanKind::Fallback;
        let fallback = evidence
            .key_span
            .and_then(|span| map_key(span, fallback_kind))
            .or_else(|| {
                source.try_map_enclosing_span(construct).map(|span| Origin {
                    span,
                    kind: fallback_kind,
                })
            });
        let mut result = Self {
            fallback,
            ..Self::default()
        };
        for entry in evidence.entries {
            control.checkpoint()?;
            let fallback = entry
                .key_span
                .and_then(|span| map_key(span, fallback_kind))
                .or(fallback);
            let mut slots = BTreeMap::new();
            for occurrence in entry.occurrences {
                control.checkpoint()?;
                let mut fields = Vec::new();
                for item in occurrence.fields {
                    control.checkpoint()?;
                    if let Some(field) = GridCoordinateField::from_key(&item.name) {
                        fields.push(FieldSource {
                            field,
                            origin: item
                                .key_span
                                .and_then(|span| map_key(span, ParseDiagnosticSpanKind::Exact))
                                .or(fallback),
                            nonfinite: item.nonfinite,
                        });
                    }
                }
                slots.insert(occurrence.index, fields);
            }
            result.insert(
                entry.id,
                PlacementSource {
                    order: 0,
                    fallback,
                    slots,
                },
            );
        }
        Ok(result)
    }

    pub(super) fn entry_order(&self, id: &str) -> Option<usize> {
        self.entries.get(id).map(|entry| entry.order)
    }

    pub(super) fn coordinate_order(
        &self,
        id: &str,
        slot: Option<usize>,
    ) -> impl Iterator<Item = (GridCoordinateField, Option<f64>)> {
        let fields = self
            .entries
            .get(id)
            .and_then(|entry| entry.slots.get(&slot));
        fields
            .into_iter()
            .flatten()
            .map(|field| (field.field, field.nonfinite))
            .chain(
                [GridCoordinateField::Column, GridCoordinateField::Row]
                    .into_iter()
                    .filter(move |field| {
                        fields.is_none_or(|fields| fields.iter().all(|item| item.field != *field))
                    })
                    .map(|field| (field, None)),
            )
    }

    pub(super) fn attach_span(
        &self,
        diagnostic: ParseDiagnostic,
        error: &GridCoordinateError<'_>,
    ) -> ParseDiagnostic {
        let origin = self
            .entries
            .get(error.placement_id)
            .and_then(|entry| {
                entry
                    .slots
                    .get(&error.occurrence_index)
                    .and_then(|fields| {
                        fields
                            .iter()
                            .find(|field| field.field == error.field)
                            .and_then(|field| field.origin)
                    })
                    .or(entry.fallback)
            })
            .or(self.fallback);
        match origin {
            Some(origin) => diagnostic.with_span(origin.span, origin.kind),
            None => diagnostic,
        }
    }

    fn insert(&mut self, id: String, mut source: PlacementSource) {
        match self.entries.entry(id) {
            Entry::Occupied(mut previous) => {
                source.order = previous.get().order;
                previous.insert(source);
            }
            Entry::Vacant(entry) => {
                source.order = self.next_order;
                self.next_order += 1;
                entry.insert(source);
            }
        }
    }

    pub(in crate::preprocess) fn replace_init_entries(
        &mut self,
        config: &Value,
        mut update: Self,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        let Some(placements) = config
            .pointer("/grid/placements")
            .and_then(Value::as_object)
        else {
            return Ok(());
        };
        for (id, _) in ordered_entries(placements, &update, control)? {
            let entry = update.entries.remove(id).unwrap_or_default();
            self.insert(id.to_owned(), entry);
        }
        self.fallback = update.fallback.or(self.fallback);
        Ok(())
    }

    fn merge_placements(
        &mut self,
        previous: &Map<String, Value>,
        update: &Map<String, Value>,
        mut incoming: Self,
        control: &OperationControl,
    ) -> OperationControlResult<()> {
        for (id, value) in ordered_entries(update, &incoming, control)? {
            control.checkpoint()?;
            let mut next = incoming.entries.remove(id).unwrap_or_default();
            let (Some(previous_value), Some(previous_source)) =
                (previous.get(id), self.entries.get_mut(id))
            else {
                self.insert(id.to_owned(), next);
                continue;
            };
            match (previous_value, value) {
                (Value::Object(_), Value::Object(_)) => {
                    previous_source.merge_fields(None, next.slots.remove(&None));
                }
                (Value::Array(previous), Value::Array(update)) => {
                    for (index, item) in update.iter().enumerate() {
                        control.checkpoint()?;
                        let slot = Some(index);
                        if previous.get(index).is_some_and(Value::is_object) && item.is_object() {
                            previous_source.merge_fields(slot, next.slots.remove(&slot));
                        } else {
                            previous_source.slots.remove(&slot);
                            if let Some(fields) = next.slots.remove(&slot) {
                                previous_source.slots.insert(slot, fields);
                            }
                        }
                    }
                }
                (Value::Array(previous), Value::Object(update)) => {
                    // A numeric property on an array addresses its existing slot.
                    for (id, value) in update {
                        control.checkpoint()?;
                        let Some(index) = js_array_index(id).map(|index| index as usize) else {
                            continue;
                        };
                        let slot = Some(index);
                        if previous.get(index).is_some_and(Value::is_object) && value.is_object() {
                            if let Some(fields) = previous_source.slots.get_mut(&slot) {
                                fields.retain(|field| value.get(field.field.as_str()).is_none());
                            }
                        } else {
                            previous_source.slots.remove(&slot);
                        }
                    }
                }
                _ => self.insert(id.to_owned(), next),
            }
        }
        self.fallback = incoming.fallback.or(self.fallback);
        Ok(())
    }
}

impl PlacementSource {
    fn merge_fields(&mut self, slot: Option<usize>, next: Option<Vec<FieldSource>>) {
        let Some(next) = next else { return };
        let fields = self.slots.entry(slot).or_default();
        for field in next {
            if let Some(previous) = fields.iter_mut().find(|item| item.field == field.field) {
                *previous = field;
            } else {
                fields.push(field);
            }
        }
    }
}

pub(in crate::preprocess) fn merge_source_grid(
    frontmatter: &MermaidConfig,
    origins: &mut GridSource,
    directives: &MermaidConfig,
    directive_origins: GridSource,
    control: &OperationControl,
) -> OperationControlResult<Option<Value>> {
    let Some(update) = directives.get("grid") else {
        return Ok(None);
    };
    let previous = frontmatter.get("grid").unwrap_or(&Value::Null);
    match (previous.get("placements"), update.get("placements")) {
        (Some(Value::Object(previous)), Some(Value::Object(update))) => {
            origins.merge_placements(previous, update, directive_origins, control)?;
        }
        (_, Some(_)) => *origins = directive_origins,
        _ => {}
    }
    let mut merged =
        ManagedJson::from_value(clone_value_nonrecursive_with_control(previous, control)?);
    merge_grid_member(merged.as_value_mut(), update, control)?;
    Ok(Some(merged.into_unmanaged_value()))
}

/// Mermaid's frontmatter/init merge uses lodash merge, so occurrence arrays merge by index.
/// This runs only on the decoder-bounded Grid source section.
fn merge_grid_member(
    destination: &mut Value,
    update: &Value,
    control: &OperationControl,
) -> OperationControlResult<()> {
    control.checkpoint()?;
    match (destination, update) {
        (Value::Object(destination), Value::Object(update)) => {
            for (key, value) in update {
                if key != "__proto__" && !(key == "constructor" && destination.get(key).is_none()) {
                    merge_grid_member(
                        destination.entry(key).or_insert(Value::Null),
                        value,
                        control,
                    )?;
                }
            }
        }
        (Value::Array(destination), Value::Array(update)) => {
            for (index, value) in update.iter().enumerate() {
                if index == destination.len() {
                    destination.push(Value::Null)
                }
                merge_grid_member(&mut destination[index], value, control)?;
            }
        }
        (Value::Array(destination), Value::Object(update)) => {
            let mut positions = BTreeMap::new();
            for (key, value) in update {
                control.checkpoint()?;
                if let Some(index) = js_array_index(key) {
                    positions.insert(index as usize, value);
                } else if key != "__proto__" {
                    // Named properties make Object.keys(array).length differ from its slot
                    // count. Preserve that rejection without storing a named array member.
                    destination.push(Value::Null);
                    return Ok(());
                }
            }
            for (index, value) in positions {
                if index > destination.len() {
                    // An explicit hole makes an occurrence sequence inadmissible. Record the
                    // hole without allocating any storage proportional to the authored index.
                    destination.push(Value::Null);
                    break;
                }
                if index == destination.len() {
                    destination.push(Value::Null)
                }
                merge_grid_member(&mut destination[index], value, control)?;
            }
        }
        (destination, update) => {
            let replacement = clone_value_nonrecursive_with_control(update, control)?;
            crate::config::replace_value_nonrecursive(destination, replacement);
        }
    }
    Ok(())
}
