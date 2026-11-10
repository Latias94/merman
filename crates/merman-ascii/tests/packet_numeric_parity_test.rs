#![cfg(feature = "diagram-packet")]

use merman_ascii::{
    AsciiError, AsciiRenderOptions, AsciiRenderer, AsciiResourceLimitId, AsciiResourcePolicy,
};
use merman_core::SemanticParser;
use merman_model::runtime::RuntimePolicy;
use merman_model::{OperationControl, ParsedSemanticDiagram};

fn source(width: &str, body: &str) -> String {
    format!("---\nconfig:\n  packet:\n    bitsPerRow: {width}\n---\npacket\n{body}\n")
}

fn render(
    parsed: &ParsedSemanticDiagram,
    control: &OperationControl,
    policy: AsciiResourcePolicy,
) -> merman_ascii::Result<String> {
    let context = RuntimePolicy::deterministic().begin_operation().unwrap();
    AsciiRenderer::new(AsciiRenderOptions::ascii())
        .unwrap()
        .render_parsed(parsed, control, &context, policy)
}

#[test]
fn packet_ascii_renders_number_ranges_without_expanding_each_bit() {
    for (width, body, expected) in [
        (
            "9007199254740994",
            "0-9007199254740993: \"x\"",
            "range=[0..9007199254740992] bits=9007199254740992",
        ),
        (
            "9007199254740996",
            "+9007199254740994: \"x\"",
            "range=[0..9007199254740992] bits=9007199254740994",
        ),
        (
            "36893488147419103232",
            "+18446744073709551616: \"x\"",
            "range=[0..18446744073709552000] bits=18446744073709552000",
        ),
        (
            "2000000000000000000000",
            "+1000000000000000000000: \"x\"",
            "range=[0..1e+21] bits=1e+21",
        ),
    ] {
        let parsed = SemanticParser::new()
            .parse_diagram_sync(&source(width, body))
            .unwrap();
        let text = render(
            &parsed,
            &OperationControl::new(),
            AsciiResourcePolicy::default(),
        )
        .unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.contains(expected), "{text}");
    }
}

#[test]
fn packet_ascii_accepts_source_split_widths_including_zero_and_fractional_bits() {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(&source("1", "+3: \"x\""))
        .unwrap();
    let text = render(
        &parsed,
        &OperationControl::new(),
        AsciiResourcePolicy::default(),
    )
    .unwrap();
    assert_eq!(text.lines().count(), 6);
    assert_eq!(text.matches("bits=0 ").count(), 3);

    let parsed = SemanticParser::new()
        .parse_diagram_sync(&source("2.5", "0-3: \"x\""))
        .unwrap();
    let text = render(
        &parsed,
        &OperationControl::new(),
        AsciiResourcePolicy::default(),
    )
    .unwrap();
    assert!(text.contains("range=[0..1.5] bits=1.5"));
    assert!(text.contains("range=[2.5..3] bits=0.5"));

    let parsed = SemanticParser::new()
        .parse_diagram_sync(&source("1.5", "0: \"a\"\n1-5: \"b\""))
        .unwrap();
    let text = render(
        &parsed,
        &OperationControl::new(),
        AsciiResourcePolicy::default(),
    )
    .unwrap();
    assert!(text.contains("range=[1..0.5] bits=-0.5"));
}

#[test]
fn packet_ascii_preserves_nonfinite_db_values_in_a_bounded_textual_view() {
    let token = "9".repeat(400);
    let parsed = SemanticParser::new()
        .parse_diagram_sync(&source(
            "1e308",
            &format!("0-{token}: \"x\"\n{token}: \"next\""),
        ))
        .unwrap();
    let text = render(
        &parsed,
        &OperationControl::new(),
        AsciiResourcePolicy::default(),
    )
    .unwrap();
    assert_eq!(text.lines().count(), 6);
    assert!(text.contains("range=[1e+308..Infinity] bits=Infinity"));
    assert!(text.contains("range=[Infinity..Infinity] bits=NaN"));
}

#[test]
fn packet_ascii_keeps_output_limits_and_cancellation_for_large_numeric_values() {
    let parsed = SemanticParser::new()
        .parse_diagram_sync(&source("9007199254740994", "0-9007199254740993: \"x\""))
        .unwrap();
    let policy = AsciiResourcePolicy::default()
        .with_limit(AsciiResourceLimitId::MaxOutputBytes, 32)
        .unwrap();
    let error = render(&parsed, &OperationControl::new(), policy).unwrap_err();
    assert!(
        matches!(error, AsciiError::ResourceLimitExceeded(ref details)
        if details.limit == AsciiResourceLimitId::MaxOutputBytes)
    );

    let control = OperationControl::new();
    control.cancel();
    assert!(matches!(
        render(&parsed, &control, AsciiResourcePolicy::default()),
        Err(AsciiError::Cancelled(_))
    ));
}
