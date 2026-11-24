use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

/// Placement hints from a successful experimental Shoal preview.
///
/// Retain the frame within one diagram editing session and pass it to the next render of
/// that diagram. Geometry is measured again from the current model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShoalLayoutFrame {
    pub(crate) family: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) groups: Vec<String>,
    pub(crate) nodes: BTreeMap<String, ShoalFrameNode>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ShoalFrameNode {
    pub(crate) parent: Option<u32>,
    pub(crate) rank: u32,
    #[serde(deserialize_with = "finite")]
    pub(crate) cross_key: f64,
    pub(crate) direction: FrameDirection,
}

impl Eq for ShoalFrameNode {}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum FrameDirection {
    Down,
    Up,
    Left,
    Right,
}

fn finite<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    let value = f64::deserialize(deserializer)?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(serde::de::Error::custom(
            "expected a finite cross coordinate",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::FrameDirection;

    #[test]
    fn frame_directions_round_trip_the_existing_wire_names() {
        for (direction, json) in [
            (FrameDirection::Down, "\"down\""),
            (FrameDirection::Up, "\"up\""),
            (FrameDirection::Left, "\"left\""),
            (FrameDirection::Right, "\"right\""),
        ] {
            assert_eq!(serde_json::to_string(&direction).unwrap(), json);
            assert_eq!(
                serde_json::from_str::<FrameDirection>(json).unwrap(),
                direction
            );
        }
    }
}
