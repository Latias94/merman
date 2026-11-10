#![cfg(feature = "diagram-packet")]

use merman_core::{Error, ParseDiagnostic, SemanticParser};
use merman_model::SemanticDiagramModel;
use merman_model::families::packet::PacketDiagramModel;

fn source(width: &str, body: &str) -> String {
    format!("---\nconfig:\n  packet:\n    bitsPerRow: {width}\n---\npacket\n{body}\n")
}

fn packet(source: &str) -> PacketDiagramModel {
    let parsed = SemanticParser::new().parse_diagram_sync(source).unwrap();
    let SemanticDiagramModel::Packet(model) = parsed.model else {
        panic!("expected Packet model");
    };
    model
}

fn diagnostic(source: &str) -> ParseDiagnostic {
    match SemanticParser::new()
        .parse_diagram_sync(source)
        .unwrap_err()
    {
        Error::DiagramParse { diagnostic, .. } => diagnostic,
        error => panic!("expected Packet diagnostic, got {error}"),
    }
}

#[test]
fn packet_rounds_integer_tokens_before_range_arithmetic() {
    let text = source("9007199254740994", "0-9007199254740993: \"x\"");
    let model = packet(&text);
    assert_eq!(model.packet.len(), 1);
    assert_eq!(model.packet[0].len(), 1);
    let block = &model.packet[0][0];
    assert_eq!(block.start, 0.0);
    assert_eq!(block.end, 9_007_199_254_740_992.0);
    assert_eq!(block.bits, 9_007_199_254_740_992.0);
    assert_eq!(block.label, "x");

    #[cfg(feature = "compat-json")]
    {
        let parsed = merman_core::MermaidEngine::new()
            .parse_diagram_sync(&text, merman_core::ParseOptions::strict())
            .unwrap()
            .unwrap();
        assert_eq!(
            parsed.model["packet"],
            serde_json::json!([[{
                "start": 0, "end": 9007199254740992_i64,
                "bits": 9007199254740992_i64, "label": "x",
            }]])
        );
    }
}

#[test]
fn packet_accepts_values_above_i64_and_u64_without_using_them_as_counts() {
    for (width, bits, expected) in [
        (
            "18446744073709551616",
            "9223372036854775808",
            9_223_372_036_854_775_808.0,
        ),
        (
            "36893488147419103232",
            "18446744073709551616",
            18_446_744_073_709_551_616.0,
        ),
    ] {
        let model = packet(&source(width, &format!("+{bits}: \"x\"")));
        assert_eq!(model.packet.len(), 1);
        assert_eq!(model.packet[0].len(), 1);
        assert_eq!(model.packet[0][0].end, expected);
        assert_eq!(model.packet[0][0].bits, expected);
    }
}

#[test]
fn packet_retains_explicit_relative_width_after_rounding_the_end() {
    let model = packet(&source("9007199254740996", "+9007199254740994: \"x\""));
    let block = &model.packet[0][0];
    assert_eq!(block.end, 9_007_199_254_740_992.0);
    assert_eq!(block.bits, 9_007_199_254_740_994.0);
    assert_ne!(block.bits, block.end - block.start + 1.0);

    // Mermaid computes a singleton's end as (start + 1) - 1, in that order.
    let error = diagnostic(&source(
        "9007199254741000",
        "0-9007199254740993: \"x\"\n9007199254740993: \"next\"",
    ));
    assert_eq!(
        error.message(),
        "Block start 9007199254740992 is greater than block end 9007199254740991."
    );
    assert_eq!(error.code(), None);

    let error = diagnostic(&source(
        "36893488147419103232",
        "+18446744073709551616: \"x\"\n18446744073709555712: \"gap\"",
    ));
    assert_eq!(
        error.message(),
        "Packet block 18446744073709556000 - 18446744073709556000 is not contiguous. It should start from 18446744073709552000."
    );
}

#[test]
fn packet_preserves_fractional_row_boundaries_and_upstream_split_widths() {
    let model = packet(&source("2.5", "0-3: \"x\""));
    let blocks: Vec<_> = model
        .packet
        .iter()
        .flatten()
        .map(|block| (block.start, block.end, block.bits))
        .collect();
    assert_eq!(blocks, [(0.0, 1.5, 1.5), (2.5, 3.0, 0.5)]);

    let model = packet("packet\n0-10: \"first\"\n11-90: \"split\"\n");
    let widths: Vec<_> = model
        .packet
        .iter()
        .flatten()
        .map(|block| block.bits)
        .collect();
    assert_eq!(widths, [11.0, 20.0, 31.0, 26.0]);
}

#[test]
fn packet_row_budget_counts_materialized_rows_including_a_final_partial_row() {
    assert_eq!(packet("packet\n0-319999: \"x\"\n").packet.len(), 10_000);
    for body in [
        "0-320000: \"x\"",
        "0-319999: \"x\"\n+1: \"extra\"",
        "0-9223372036854775808: \"grammar accepts this integer\"",
    ] {
        let error = diagnostic(&format!("packet\n{body}\n"));
        assert_eq!(error.code(), Some("merman.packet.row_limit"));
        assert_eq!(
            error.message(),
            "Packet exceeds the 10000-row safety limit."
        );
    }
}

#[test]
fn packet_rejects_nonprogress_before_repeating_a_rounded_split() {
    // Probed one arithmetic step only: (width - 1) + 1 != width. Running the upstream
    // population loop on this case would repeat the same remainder without advancing its row.
    let error = diagnostic(&source("9007199254740994", "0-9007199254740996: \"x\""));
    assert_eq!(error.code(), Some("merman.packet.nonprogress"));
    assert!(error.message().contains("cannot make progress"));
    assert!(error.span().is_some());
}

#[test]
fn packet_nonfinite_number_states_survive_a_bounded_source_and_canonical_roundtrip() {
    let token = "9".repeat(400);
    let model = packet(&source(
        "1e308",
        &format!("0-{token}: \"x\"\n{token}: \"next\""),
    ));
    assert_eq!(model.packet.len(), 3);
    assert_eq!(model.packet[1][0].end, f64::INFINITY);
    assert_eq!(model.packet[2][0].start, f64::INFINITY);
    assert!(model.packet[2][0].bits.is_nan());

    let wire = serde_json::to_value(&model).unwrap();
    assert_eq!(wire["packet"][1][0]["end"], "Infinity");
    assert_eq!(wire["packet"][2][0]["bits"], "NaN");
    let roundtrip: PacketDiagramModel = serde_json::from_value(wire).unwrap();
    assert_eq!(roundtrip.packet[1][0].end, f64::INFINITY);
    assert!(roundtrip.packet[2][0].bits.is_nan());
}

#[test]
fn packet_number_semantics_do_not_widen_the_int_grammar() {
    for token in ["01", "1.0", "1e2", "-1", "Infinity"] {
        let error = diagnostic(&format!("packet\n+{token}: \"x\"\n"));
        assert!(
            error.message().contains("unexpected packet statement"),
            "{token}: {error:?}"
        );
        assert_eq!(error.code(), None);
    }
}
