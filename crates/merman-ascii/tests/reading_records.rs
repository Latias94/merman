#![cfg(any(feature = "diagram-architecture", feature = "diagram-agentflow"))]

mod support;

#[cfg(feature = "diagram-agentflow")]
use merman_ascii::{AsciiColorMode, TerminalWidthProfile};
use merman_ascii::{
    AsciiOutput, AsciiOutputEncoding, AsciiOutputOutcome, AsciiProjection, AsciiRenderOptions,
    AsciiRenderer, AsciiResourcePolicy, AsciiViewportPolicy, Lossiness, OverflowPolicy,
};
use merman_core::SemanticParser;
use merman_model::{OperationControl, ParsedSemanticDiagram, runtime::RuntimePolicy};

fn parse(source: &str) -> ParsedSemanticDiagram {
    SemanticParser::new()
        .parse_diagram_sync(source)
        .expect("reading-record source parses")
}

fn render(
    parsed: &ParsedSemanticDiagram,
    options: AsciiRenderOptions,
    viewport: AsciiViewportPolicy,
) -> AsciiOutput {
    let operation = RuntimePolicy::deterministic().begin_operation().unwrap();
    AsciiRenderer::new(options)
        .unwrap()
        .render_parsed_report(
            parsed,
            viewport,
            &OperationControl::new(),
            &operation,
            AsciiResourcePolicy::default(),
        )
        .unwrap()
}

fn assert_primary(output: &AsciiOutput, projection: AsciiProjection, width: usize) {
    assert_eq!(output.projection, projection, "{}", output.text);
    assert_eq!(output.outcome, AsciiOutputOutcome::Primary);
    assert_eq!(output.requested_max_width, Some(width));
    assert_eq!(output.primary_extent, Some(output.emitted_extent));
    assert_eq!(output.primary_overflowed, Some(false));
    assert!(!output.emitted_overflowed);
    assert!(!output.fallback.attempted);
    assert_eq!(output.fallback.reason, None);
    assert_eq!(output.lossiness, Lossiness::None);
    assert!(output.emitted_extent.width <= width, "{}", output.text);
    if output.encoding == AsciiOutputEncoding::Plain {
        assert_eq!(
            support::terminal_extent_with_profile(&output.text, output.width_profile),
            (output.emitted_extent.width, output.emitted_extent.height)
        );
    }
}

fn unfolded_records(text: &str) -> String {
    // Remove continuation indentation while retaining authored spaces inside fields.
    text.lines()
        .map(|line| line.strip_prefix("  ").unwrap_or(line))
        .collect()
}

#[cfg(feature = "diagram-architecture")]
#[test]
fn architecture_records_reflow_without_wrapping_the_spatial_overview() {
    let parsed = parse(
        "architecture-beta\nservice a(server)[Gateway]\nservice b(database)[Storage]\na:R --> L:b\n",
    );
    let options = AsciiRenderOptions::ascii();
    let natural = render(&parsed, options, AsciiViewportPolicy::unrestricted());
    assert_eq!(natural.projection, AsciiProjection::Diagrammatic);
    let (overview, records) = natural.text.split_once("Architecture records").unwrap();
    let overview_width = support::terminal_extent_with_profile(overview, natural.width_profile).0;
    assert!(overview_width <= 60, "fixture overview must fit 60 columns");
    assert!(
        natural.emitted_extent.width > 60,
        "fixture records must need reflow"
    );
    let expected = unfolded_records(records);
    for field in [
        r#"N1 [service] id="a" title="Gateway" icon="server""#,
        r#"N2 [service] id="b" title="Storage" icon="database""#,
        r#"E1 lhs=[N1] id="a" lhsDir="R""#,
        r#"E1 rhs=[N2] id="b" rhsDir="L""#,
    ] {
        assert!(
            expected.contains(field),
            "missing {field}:\n{}",
            natural.text
        );
    }

    for width in [60, 80, 100, 120] {
        let output = render(
            &parsed,
            options,
            AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback),
        );
        assert_primary(&output, AsciiProjection::Diagrammatic, width);
        assert_eq!(output.encoding, AsciiOutputEncoding::Plain);
        let (actual_overview, records) = output.text.split_once("Architecture records").unwrap();
        assert_eq!(
            actual_overview, overview,
            "overview changed at width {width}"
        );
        assert_eq!(unfolded_records(records), expected, "width {width}");
        if width == 80 {
            assert_eq!(output.text, natural.text);
        }
    }

    // A viewport narrower than the picture must report overflow, preserving whole rows.
    let width = overview_width - 1;
    let overflow = render(&parsed, options, AsciiViewportPolicy::with_max_width(width));
    assert_eq!(overflow.projection, AsciiProjection::Diagrammatic);
    assert_eq!(overflow.outcome, AsciiOutputOutcome::WideAllowed);
    assert_eq!(overflow.primary_overflowed, Some(true));
    assert!(overflow.emitted_overflowed);
    assert!(!overflow.fallback.attempted);
    let (actual_overview, records) = overflow.text.split_once("Architecture records").unwrap();
    assert_eq!(actual_overview, overview);
    assert_eq!(overflow.emitted_extent.width, overview_width);
    assert!(support::terminal_extent_with_profile(records, overflow.width_profile).0 <= width);
    assert_eq!(unfolded_records(records), expected);
}

#[cfg(feature = "diagram-agentflow")]
const INSTRUCTION: &str = concat!(
    "Retain  two spaces <script>",
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210",
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
    "</script>",
);

#[cfg(feature = "diagram-agentflow")]
fn agentflow() -> ParsedSemanticDiagram {
    parse(&format!(
        "agentflow-beta TB\n\
         A[\"客户 <&>\"]@{{instruction: \"{INSTRUCTION}\", params: {{attempts: [1, false, null]}}}}\n\
         B[Archive]\n\
         A handoff@-->|store| B\n\
         handoff@{{retry: 2}}\n"
    ))
}

#[cfg(feature = "diagram-agentflow")]
#[test]
fn agentflow_metadata_reflows_as_a_structured_primary_at_each_requested_width() {
    let parsed = agentflow();
    let options = AsciiRenderOptions::ascii();
    let natural = render(&parsed, options, AsciiViewportPolicy::unrestricted());
    assert_eq!(natural.projection, AsciiProjection::StructuredText);
    let expected = unfolded_records(&natural.text);
    for field in [
        r#"N1 [task] id="A" label="客户 <&>""#,
        r#"N2 [task] id="B" label="Archive""#,
        r#"E1 [sequence -->] source=[N1] id="A" target=[N2] id="B"; id="handoff" label="store" explicit-id=true"#,
        r#""params":{"attempts":[1,false,null]}"#,
        r#""retry":2"#,
    ] {
        assert!(
            expected.contains(field),
            "missing {field}:\n{}",
            natural.text
        );
    }
    assert!(expected.contains(&format!(r#""instruction":"{INSTRUCTION}""#)));

    for width in [60, 80, 100, 120] {
        let output = render(
            &parsed,
            options,
            AsciiViewportPolicy::with_max_width(width).overflow(OverflowPolicy::Fallback),
        );
        assert_primary(&output, AsciiProjection::StructuredText, width);
        assert_eq!(output.encoding, AsciiOutputEncoding::Plain);
        assert_eq!(
            output.emitted_extent.width, width,
            "long metadata must use width {width}"
        );
        assert_eq!(unfolded_records(&output.text), expected, "width {width}");
        if width == 80 {
            assert_eq!(output.text, natural.text);
        }
    }
}

#[cfg(feature = "diagram-agentflow")]
#[test]
fn agentflow_cjk_records_escape_html_after_reflow_without_changing_extent() {
    let parsed = agentflow();
    let options =
        AsciiRenderOptions::unicode().with_terminal_width_profile(TerminalWidthProfile::Cjk);
    let viewport = AsciiViewportPolicy::with_max_width(60);
    let plain = render(
        &parsed,
        options,
        viewport.overflow(OverflowPolicy::Fallback),
    );
    let html = render(
        &parsed,
        options.with_color_mode(AsciiColorMode::Html),
        viewport,
    );
    assert_primary(&plain, AsciiProjection::StructuredText, 60);
    assert_primary(&html, AsciiProjection::StructuredText, 60);
    assert_eq!(html.encoding, AsciiOutputEncoding::Html);
    assert_eq!(html.width_profile, TerminalWidthProfile::Cjk);
    assert_eq!(html.primary_extent, plain.primary_extent);
    assert_eq!(html.emitted_extent, plain.emitted_extent);
    assert!(!html.text.contains('<'), "{}", html.text);
    assert!(html.text.contains("客户 &lt;&amp;&gt;"));
    let logical = html
        .text
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&");
    assert_eq!(logical, plain.text);
    assert!(unfolded_records(&logical).contains(INSTRUCTION));
}
