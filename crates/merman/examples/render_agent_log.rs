use merman::ascii::{
    AsciiLayoutProfile, AsciiOutputOutcome, AsciiRenderOptions, AsciiViewportPolicy, OverflowPolicy,
};
use merman::{AsciiRequest, OperationControl, RenderOutput, RenderRequest, Renderer};

const SOURCE: &str = r#"flowchart LR
    A[Read the complete Mermaid source] --> B[Return the complete rendered artifact]
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // The host supplies display cells, never bytes or a detected terminal width.
    let max_width = std::env::args()
        .nth(1)
        .map(|value| value.parse::<usize>())
        .transpose()?
        .unwrap_or(80);
    let output = Renderer::new().render(RenderRequest::ascii(
        SOURCE,
        OperationControl::new(),
        AsciiRequest {
            options: AsciiRenderOptions::unicode().with_layout_profile(AsciiLayoutProfile::Auto),
            viewport: AsciiViewportPolicy::with_max_width(max_width)
                .overflow(OverflowPolicy::Fallback),
            ..Default::default()
        },
    ))?;
    let RenderOutput::Ascii(Some(report)) = output else {
        return Err("no Mermaid diagram detected".into());
    };

    // Use typed metadata for decisions. Keep diagnostics off the JSON channel.
    match report.outcome {
        AsciiOutputOutcome::Primary => eprintln!("selected primary terminal output"),
        AsciiOutputOutcome::Fallback => eprintln!("selected complete structured fallback"),
        AsciiOutputOutcome::WideAllowed => eprintln!("host permitted wide terminal output"),
        _ => eprintln!("selected another supported output outcome"),
    }
    // Errors propagate before stdout is published; cancellation and quotas never trigger retries.
    println!("{}", serde_json::to_string(&report.report())?);
    Ok(())
}
