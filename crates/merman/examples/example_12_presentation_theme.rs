mod support;

use merman::svg::{
    CssOverridePolicy, HeadlessRenderer, HostTheme, HostThemeAppearance, Presentation,
    SvgOutputPolicy, SvgPipelinePreset, ThemeRole,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = support::read_mermaid_or_default(
        "example_12_presentation_theme",
        r#"sequenceDiagram
    participant Host
    participant Merman
    Host->>Merman: Render preview
    Note over Host,Merman: Presentation theme
"#,
    )?;

    let theme = HostTheme::new()
        .with_appearance(HostThemeAppearance::Dark)
        .try_with_font_family("Inter, system-ui, sans-serif")?
        .try_with_role(ThemeRole::Canvas, "#0f172a")?
        .try_with_role(ThemeRole::Surface, "#111827")?
        .try_with_role(ThemeRole::SurfaceAlt, "#1f2937")?
        .try_with_role(ThemeRole::Text, "#e5e7eb")?
        .try_with_role(ThemeRole::Border, "#475569")?
        .try_with_role(ThemeRole::Line, "#94a3b8")?
        .try_with_role(ThemeRole::NoteBackground, "#422006")?
        .try_with_role(ThemeRole::NoteBorder, "#f59e0b")?
        .try_with_role(ThemeRole::NoteText, "#fef3c7")?
        .try_with_series_palette(["#60a5fa", "#34d399", "#f59e0b"])?;
    let output = SvgOutputPolicy {
        preset: SvgPipelinePreset::ResvgSafe,
        css_override_policy: CssOverridePolicy::StripExistingImportant,
        root_background_color: Some("#0f172a".to_string()),
        drop_native_duplicate_fallbacks: false,
        scoped_css: None,
    };

    let renderer = HeadlessRenderer::new()
        .with_presentation(Presentation::new().with_theme(theme))
        .with_svg_pipeline(output.pipeline())
        .with_vendored_text_measurer()
        .with_diagram_id("presentation-theme-example");

    let Some(svg) = renderer.render_svg_sync(&input)? else {
        return Err("no Mermaid diagram detected".into());
    };

    print!("{svg}");
    Ok(())
}
