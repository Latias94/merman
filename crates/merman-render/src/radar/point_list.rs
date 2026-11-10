use crate::model::LayoutPoint;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct Point {
    #[serde(with = "layout_number")]
    x: f64,
    #[serde(with = "layout_number")]
    y: f64,
}

pub fn serialize<S: serde::Serializer>(
    points: &[LayoutPoint],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut sequence = serializer.serialize_seq(Some(points.len()))?;
    for point in points {
        sequence.serialize_element(&Point {
            x: point.x,
            y: point.y,
        })?;
    }
    sequence.end()
}

pub fn deserialize<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<LayoutPoint>, D::Error> {
    Ok(Vec::<Point>::deserialize(deserializer)?
        .into_iter()
        .map(|point| LayoutPoint {
            x: point.x,
            y: point.y,
        })
        .collect())
}

mod layout_number {
    pub use merman_model::number::serde_number::deserialize;

    pub fn serialize<S: serde::Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
        if value.is_finite() {
            // Layout JSON already exposes finite coordinates as floats, including -0.0.
            serializer.serialize_f64(*value)
        } else {
            merman_model::number::serde_number::serialize(value, serializer)
        }
    }
}

pub mod radius {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize)]
    #[serde(transparent)]
    struct Number(#[serde(with = "super::layout_number")] f64);

    pub fn serialize<S: serde::Serializer>(
        value: &Option<f64>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        value.map(Number).serialize(serializer)
    }

    pub fn deserialize<'de, D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<f64>, D::Error> {
        Ok(Option::<Number>::deserialize(deserializer)?.map(|value| value.0))
    }
}
