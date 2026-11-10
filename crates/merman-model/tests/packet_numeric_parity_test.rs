#![cfg(feature = "diagram-packet")]

use merman_model::families::packet::{
    PacketBlock, PacketDiagramModel, validate_packet_bits_per_row,
};

#[test]
fn packet_row_width_uses_the_number_schema_without_an_integer_ceiling() {
    assert_eq!(validate_packet_bits_per_row(None).unwrap(), 32.0);
    for width in [
        1.0,
        2.5,
        9_007_199_254_740_994.0,
        18_446_744_073_709_551_616.0,
    ] {
        assert_eq!(validate_packet_bits_per_row(Some(width)).unwrap(), width);
    }
    for width in [0.0, -1.0, 0.5, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        assert!(validate_packet_bits_per_row(Some(width)).is_err());
    }
}

#[test]
fn packet_canonical_numbers_roundtrip_nonfinite_states_without_null() {
    let model = PacketDiagramModel {
        packet: vec![vec![PacketBlock {
            start: f64::NEG_INFINITY,
            end: f64::INFINITY,
            bits: f64::NAN,
            label: "retained DB state".to_string(),
        }]],
        ..Default::default()
    };
    let wire = serde_json::to_value(&model).unwrap();
    assert_eq!(
        wire["packet"][0][0],
        serde_json::json!({
            "start": "-Infinity", "end": "Infinity", "bits": "NaN", "label": "retained DB state",
        })
    );
    let roundtrip: PacketDiagramModel = serde_json::from_value(wire).unwrap();
    let block = &roundtrip.packet[0][0];
    assert_eq!(block.start, f64::NEG_INFINITY);
    assert_eq!(block.end, f64::INFINITY);
    assert!(block.bits.is_nan());

    #[cfg(feature = "compat-json")]
    {
        use merman_model::families::packet::{PacketOutputData, compat};
        let compat = compat::project(
            &roundtrip,
            PacketOutputData::Model,
            "packet",
            &merman_model::MermaidConfig::empty_object(),
        );
        assert_eq!(
            compat["packet"][0][0],
            serde_json::json!({
                "start": null, "end": null, "bits": null, "label": "retained DB state",
            })
        );
    }
}

#[test]
fn packet_canonical_number_strings_are_explicit_and_finite_values_stay_numeric() {
    let model: PacketDiagramModel = serde_json::from_value(serde_json::json!({
        "title": null, "accTitle": null, "accDescr": null,
        "packet": [[{"start": 0, "end": 9007199254740992_i64, "bits": 9007199254740994_i64, "label": "x"}]],
    })).unwrap();
    let wire = serde_json::to_value(model).unwrap();
    assert_eq!(wire["packet"][0][0]["end"], 9007199254740992_i64);
    assert_eq!(wire["packet"][0][0]["bits"], 9007199254740994_i64);

    for invalid in [
        serde_json::Value::Null,
        serde_json::json!("inf"),
        serde_json::json!("9007199254740993"),
    ] {
        let mut wire = wire.clone();
        wire["packet"][0][0]["end"] = invalid;
        assert!(serde_json::from_value::<PacketDiagramModel>(wire).is_err());
    }
}

#[test]
fn packet_canonical_numbers_retain_negative_zero_before_compat_projection() {
    let model = PacketDiagramModel {
        packet: vec![vec![PacketBlock {
            start: -0.0,
            end: 0.0,
            bits: 1.0,
            label: "zero".to_string(),
        }]],
        ..Default::default()
    };
    let wire = serde_json::to_value(&model).unwrap();
    assert!(
        wire["packet"][0][0]["start"]
            .as_f64()
            .unwrap()
            .is_sign_negative()
    );
    let roundtrip: PacketDiagramModel = serde_json::from_value(wire).unwrap();
    assert!(roundtrip.packet[0][0].start.is_sign_negative());
    #[cfg(feature = "compat-json")]
    {
        use merman_model::families::packet::{PacketOutputData, compat};
        let compat = compat::project(
            &roundtrip,
            PacketOutputData::Model,
            "packet",
            &merman_model::MermaidConfig::empty_object(),
        );
        assert_eq!(compat["packet"][0][0]["start"], 0);
    }
}
