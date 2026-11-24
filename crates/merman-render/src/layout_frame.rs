use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
#[cfg(feature = "layout-shoal")]
use std::sync::Mutex;

#[derive(Debug, Default, Clone)]
pub(crate) struct GraphPlacement {
    pub(crate) adaptation: Option<crate::LayoutAdaptationObservation>,
    pub(crate) frame: Option<Arc<ShoalLayoutFrame>>,
}

/// Placement hints from a successful experimental Shoal preview.
///
/// Retain the frame within one diagram editing session and pass it to the next render of
/// that diagram. Geometry is measured again from the current model. An in-memory frame can
/// also transfer its reusable layout storage to the next render.
#[derive(Clone, Serialize, Deserialize)]
pub struct ShoalLayoutFrame {
    pub(crate) family: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) groups: Vec<String>,
    pub(crate) nodes: BTreeMap<String, ShoalFrameNode>,
    #[cfg(feature = "layout-shoal")]
    #[serde(skip)]
    pub(crate) scratch: Option<Arc<Mutex<Option<merman_layout_shoal::LayoutScratch>>>>,
}

impl fmt::Debug for ShoalLayoutFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShoalLayoutFrame")
            .field("family", &self.family)
            .field("groups", &self.groups)
            .field("nodes", &self.nodes)
            .finish()
    }
}

impl PartialEq for ShoalLayoutFrame {
    fn eq(&self, other: &Self) -> bool {
        self.family == other.family && self.groups == other.groups && self.nodes == other.nodes
    }
}

impl Eq for ShoalLayoutFrame {}

#[cfg(feature = "layout-shoal")]
impl ShoalLayoutFrame {
    pub(crate) fn take_scratch(&self) -> Option<merman_layout_shoal::LayoutScratch> {
        // Concurrent replays of the same frame can always compute a fresh layout.
        self.scratch.as_ref()?.try_lock().ok()?.take()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ShoalFrameNode {
    pub(crate) parent: Option<u32>,
    pub(crate) rank: u32,
    #[serde(deserialize_with = "finite")]
    pub(crate) cross_key: f64,
    pub(crate) direction: FrameDirection,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) position: Option<ShoalFramePosition>,
}

impl Eq for ShoalFrameNode {}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ShoalFramePosition {
    #[serde(deserialize_with = "finite")]
    pub(crate) x: f64,
    #[serde(deserialize_with = "finite")]
    pub(crate) y: f64,
    #[serde(deserialize_with = "finite")]
    pub(crate) width: f64,
    #[serde(deserialize_with = "finite")]
    pub(crate) height: f64,
    pub(crate) component: FrameSignature,
    pub(crate) incident_signature: FrameSignature,
}

/// Each word is exactly representable by JSON consumers using JavaScript numbers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct FrameSignature([u32; 2]);

#[cfg(any(feature = "layout-shoal", test))]
impl FrameSignature {
    pub(crate) fn from_bits(bits: u64) -> Self {
        Self([bits as u32, (bits >> 32) as u32])
    }

    pub(crate) fn bits(self) -> u64 {
        u64::from(self.0[0]) | (u64::from(self.0[1]) << 32)
    }
}

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
            "expected finite placement geometry",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameDirection, FrameSignature, ShoalLayoutFrame};

    #[test]
    fn previous_frames_still_decode_and_signature_words_round_trip() {
        let frame: ShoalLayoutFrame = serde_json::from_str(
            r#"{"family":"flowchart","nodes":{"A":{"parent":null,"rank":0,"cross_key":20,"direction":"down"}}}"#,
        )
        .unwrap();
        assert!(frame.nodes["A"].position.is_none());

        let bits = 0xff00_ffff_f0ff_ffaf;
        let wire = serde_json::to_string(&FrameSignature::from_bits(bits)).unwrap();
        let words: [f64; 2] = serde_json::from_str(&wire).unwrap();
        assert!(words.into_iter().all(|word| word == f64::from(word as u32)));
        assert_eq!(
            serde_json::from_str::<FrameSignature>(&wire)
                .unwrap()
                .bits(),
            bits
        );
    }

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
