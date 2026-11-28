//! Decode source objects with literal keys while retaining each format's scalar rules.

use super::grid_config::{
    GridPlacementFieldSource, GridPlacementOccurrenceSource, GridPlacementSource,
    GridPlacementsSource,
};
use serde::de::{Deserialize, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fmt;
use std::ops::Range;

pub(super) fn parse_json5_value(
    input: &str,
    capture_grid: bool,
) -> (Option<Value>, Option<GridPlacementsSource>) {
    if capture_grid {
        json5::from_str::<LiteralJson5<true>>(input)
            .ok()
            .map_or((None, None), |value| (Some(value.0), value.1))
    } else {
        (
            json5::from_str::<LiteralJson5<false>>(input)
                .ok()
                .map(|v| v.0),
            None,
        )
    }
}

struct LiteralJson5<const GRID: bool>(Value, Option<GridPlacementsSource>);

impl<'de, const GRID: bool> Deserialize<'de> for LiteralJson5<GRID> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut grid = None;
        let value = Json5GridCapture::init(GRID, &mut grid).deserialize(deserializer)?;
        Ok(Self(value, grid))
    }
}

impl<'de> DeserializeSeed<'de> for Json5GridCapture<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(Json5Visitor { grid: self })
    }
}

struct Json5Visitor<'a> {
    grid: Json5GridCapture<'a>,
}

impl<'de> Visitor<'de> for Json5Visitor<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON5 value")
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
        // Ordinary directives have always mapped JSON5 non-finite numbers to null.
        self.grid.record_nonfinite(value);
        Ok(value.into())
    }

    fn visit_str<E>(self, value: &str) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_string<E>(self, value: String) -> Result<Value, E> {
        Ok(value.into())
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut grid = self.grid;
        let mut values = Vec::new();
        loop {
            let mut occurrence = GridPlacementOccurrenceSource {
                index: Some(values.len()),
                fields: Vec::new(),
            };
            let Some(value) = sequence.next_element_seed(grid.item(&mut occurrence))? else {
                break;
            };
            grid.finish_item(occurrence, &value);
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut grid = self.grid.object();
        let mut slots = HashMap::new();
        let mut values = Map::new();
        // Value's default visitor classifies the first key as a possible Serde marker.
        while let Some(key) = object.next_key_seed(Json5KeySeed)? {
            let value = object.next_value_seed(grid.member(&key, &mut slots))?;
            values.insert(key.name, value);
        }
        Ok(Value::Object(values))
    }
}

/// Tracks one branch of the Grid shape while the ordinary JSON5 visitor owns its values.
pub(super) enum Json5GridCapture<'a> {
    Ignore,
    Init(&'a mut Option<GridPlacementsSource>),
    Config(&'a mut Option<GridPlacementsSource>),
    Placements(&'a mut GridPlacementsSource),
    Placement(&'a mut GridPlacementSource),
    Occurrence(&'a mut GridPlacementOccurrenceSource),
    Coordinate(&'a mut Option<f64>),
}

impl<'a> Json5GridCapture<'a> {
    pub(super) fn init(enabled: bool, output: &'a mut Option<GridPlacementsSource>) -> Self {
        if enabled {
            Self::Init(output)
        } else {
            Self::Ignore
        }
    }

    pub(super) fn object(self) -> Self {
        match self {
            Self::Placement(entry) => {
                entry
                    .occurrences
                    .push(GridPlacementOccurrenceSource::default());
                Self::Occurrence(&mut entry.occurrences[0])
            }
            grid => grid,
        }
    }

    pub(super) fn member<'b>(
        &'b mut self,
        key: &Json5Key,
        slots: &mut HashMap<String, usize>,
    ) -> Json5GridCapture<'b> {
        match self {
            Self::Init(output) if key.name == "grid" => {
                **output = None;
                Json5GridCapture::Config(output)
            }
            Self::Config(output) if key.name == "placements" => {
                Json5GridCapture::Placements(output.insert(GridPlacementsSource {
                    key_span: key.addresses.clone(),
                    entries: Vec::new(),
                }))
            }
            Self::Placements(placements) => {
                let entry = write_ordered_member(
                    &mut placements.entries,
                    slots,
                    &key.name,
                    GridPlacementSource {
                        id: key.name.clone(),
                        key_span: key.addresses.clone(),
                        occurrences: Vec::new(),
                    },
                );
                Json5GridCapture::Placement(entry)
            }
            Self::Occurrence(occurrence) => {
                let field = write_ordered_member(
                    &mut occurrence.fields,
                    slots,
                    &key.name,
                    GridPlacementFieldSource {
                        name: key.name.clone(),
                        key_span: key.addresses.clone(),
                        nonfinite: None,
                    },
                );
                if matches!(key.name.as_str(), "row" | "column") {
                    Json5GridCapture::Coordinate(&mut field.nonfinite)
                } else {
                    Json5GridCapture::Ignore
                }
            }
            _ => Json5GridCapture::Ignore,
        }
    }

    pub(super) fn item<'b>(
        &self,
        occurrence: &'b mut GridPlacementOccurrenceSource,
    ) -> Json5GridCapture<'b> {
        if matches!(self, Self::Placement(_)) {
            Json5GridCapture::Occurrence(occurrence)
        } else {
            Json5GridCapture::Ignore
        }
    }

    pub(super) fn finish_item(&mut self, occurrence: GridPlacementOccurrenceSource, value: &Value) {
        if let Self::Placement(entry) = self
            && value.is_object()
        {
            entry.occurrences.push(occurrence);
        }
    }

    pub(super) fn record_nonfinite(self, value: f64) -> bool {
        if let Self::Coordinate(output) = self
            && !value.is_finite()
        {
            *output = Some(value);
            return true;
        }
        false
    }
}

fn write_ordered_member<'a, T>(
    members: &'a mut Vec<T>,
    slots: &mut HashMap<String, usize>,
    key: &str,
    member: T,
) -> &'a mut T {
    // Like JSON5 object assignment, a duplicate keeps its first slot and last value.
    let index = *slots.entry(key.to_owned()).or_insert(members.len());
    if index == members.len() {
        members.push(member);
    } else {
        members[index] = member;
    }
    &mut members[index]
}

pub(super) struct Json5KeySeed;

pub(super) struct Json5Key {
    pub(super) name: String,
    // The plain Deserialize entry point has no access to its input. These addresses are
    // rebased to a byte range while the input is alive, before the evidence is exposed.
    pub(super) addresses: Option<Range<usize>>,
}

impl<'de> DeserializeSeed<'de> for Json5KeySeed {
    type Value = Json5Key;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Json5Key, D::Error> {
        deserializer.deserialize_identifier(Json5KeyVisitor)
    }
}

struct Json5KeyVisitor;

impl<'de> Visitor<'de> for Json5KeyVisitor {
    type Value = Json5Key;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON5 object key")
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Json5Key, E> {
        let start = value.as_ptr() as usize;
        Ok(Json5Key {
            name: value.to_owned(),
            addresses: start.checked_add(value.len()).map(|end| start..end),
        })
    }

    fn visit_str<E>(self, value: &str) -> Result<Json5Key, E> {
        Ok(Json5Key {
            name: value.to_owned(),
            addresses: None,
        })
    }

    fn visit_string<E>(self, value: String) -> Result<Json5Key, E> {
        Ok(Json5Key {
            name: value,
            addresses: None,
        })
    }
}

pub(super) fn key_span_in(input: &str, addresses: Range<usize>) -> Option<Range<usize>> {
    let start = input.as_ptr() as usize;
    let range = addresses.start.checked_sub(start)?..addresses.end.checked_sub(start)?;
    input.get(range.clone()).map(|_| range)
}
