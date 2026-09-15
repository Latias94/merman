use merman::ascii::{
    AsciiColorMode, AsciiColorTheme, AsciiLayoutProfile, AsciiRenderOptions, AsciiRgb,
    AsciiTerminalPalette, AsciiViewportPolicy, OverflowPolicy,
};
use merman::{AsciiRequest, OperationControl, RenderOutput, RenderRequest, Renderer};

const SOURCE: &str = r#"flowchart LR
    Host[Host palette] --> Renderer[Terminal roles] --> Text[Styled text]
"#;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // These colors belong to the application; Merman does not query terminal appearance.
    let palette = AsciiTerminalPalette::new(
        AsciiRgb::from_hex24(0xe5e7eb),
        AsciiRgb::from_hex24(0x0f172a),
    )
    .with_line(AsciiRgb::from_hex24(0x94a3b8))
    .with_accent(AsciiRgb::from_hex24(0x60a5fa));
    let options = AsciiRenderOptions::unicode()
        .with_layout_profile(AsciiLayoutProfile::Auto)
        .with_color_mode(AsciiColorMode::TrueColor)
        .with_color_theme(AsciiColorTheme::from_terminal_palette(palette));
    let output = Renderer::new().render(RenderRequest::ascii(
        SOURCE,
        OperationControl::new(),
        AsciiRequest {
            options,
            // Styled fallback is not admitted. The host chooses how to handle width errors.
            viewport: AsciiViewportPolicy::with_max_width(80).overflow(OverflowPolicy::Error),
            ..Default::default()
        },
    ))?;
    let RenderOutput::Ascii(Some(report)) = output else {
        return Err("no Mermaid diagram detected".into());
    };

    print!("{}", report.text);
    Ok(())
}
