//! Canonical Packet data and declared full-output facts.

pub const DEFAULT_PACKET_BITS_PER_ROW: f64 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
#[error("packet.bitsPerRow must be a finite number greater than or equal to 1; received {value}")]
pub struct PacketBitsPerRowError {
    pub value: f64,
}

/// Validates the shared Packet row-width configuration before either semantic construction or
/// layout consumes it. Mermaid's configuration schema accepts numbers, including fractions, with
/// a minimum of one. The width is never an allocation count.
pub fn validate_packet_bits_per_row(
    value: Option<f64>,
) -> std::result::Result<f64, PacketBitsPerRowError> {
    let value = value.unwrap_or(DEFAULT_PACKET_BITS_PER_ROW);
    if !value.is_finite() || value < 1.0 {
        return Err(PacketBitsPerRowError { value });
    }
    Ok(value)
}

/// Packet bit ranges grouped by the admitted row-width configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct PacketDiagramModel {
    pub title: Option<String>,
    #[serde(rename = "accTitle")]
    pub acc_title: Option<String>,
    #[serde(rename = "accDescr")]
    pub acc_descr: Option<String>,
    pub packet: Vec<Vec<PacketBlock>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PacketBlock {
    /// JavaScript Number values after grammar conversion and row splitting.
    #[serde(with = "crate::number::serde_number")]
    pub start: f64,
    #[serde(with = "crate::number::serde_number")]
    pub end: f64,
    /// Retained DB width; upstream split widths need not equal `end - start + 1`.
    #[serde(with = "crate::number::serde_number")]
    pub bits: f64,
    pub label: String,
}

/// Declared compatibility output shape retained independently of JSON projection.
///
/// Full source construction distinguishes empty input from a header-only model. Manual output
/// defaults to `Model`; this declaration does not authenticate source admission.
#[cfg(feature = "presentation-data")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PacketOutputData {
    Empty,
    #[default]
    Model,
}

#[cfg(test)]
mod wire_tests {
    use super::*;

    #[test]
    fn canonical_wire_stays_flat_and_requires_packet_rows() {
        let expected = serde_json::json!({"title": "Retained", "accTitle": null, "accDescr": null, "packet": []});
        let model: PacketDiagramModel = serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(serde_json::to_value(&model).unwrap(), expected);
        assert!(
            serde_json::from_value::<PacketDiagramModel>(serde_json::json!({"title": "Retained"}))
                .is_err()
        );
        let roundtrip: PacketDiagramModel = serde_json::from_value(expected.clone()).unwrap();
        assert_eq!(serde_json::to_value(roundtrip).unwrap(), expected);
    }
}

#[cfg(feature = "compat-json")]
pub mod compat;
