use crate::common::{
    BindingError, BindingStatus, binding_fixed_local_offset_minutes, binding_fixed_today,
    binding_site_config, no_diagram_error, parse_options, source_text,
};

pub fn render_ascii(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = source_text(source)?;
    let options = parse_options(options_json)?;
    let renderer = build_ascii_renderer(&options)?;

    render_ascii_with_renderer(&renderer, source)
}

#[derive(Clone)]
pub(crate) struct CachedAsciiEngine {
    renderer: merman::ascii::HeadlessAsciiRenderer,
}

impl CachedAsciiEngine {
    pub(crate) fn new(options: &crate::common::BindingOptions) -> Result<Self, BindingError> {
        Ok(Self {
            renderer: build_ascii_renderer(options)?,
        })
    }

    pub(crate) fn render_ascii(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        let source = source_text(source)?;
        render_ascii_with_renderer(&self.renderer, source)
    }
}

fn build_ascii_renderer(
    options: &crate::common::BindingOptions,
) -> Result<merman::ascii::HeadlessAsciiRenderer, BindingError> {
    let parse = if options
        .analysis
        .parse
        .as_ref()
        .and_then(|parse| parse.suppress_errors)
        .unwrap_or(false)
    {
        merman::ParseOptions::lenient()
    } else {
        merman::ParseOptions::strict()
    };

    let mut renderer = merman::ascii::HeadlessAsciiRenderer::new()
        .with_fixed_today(binding_fixed_today(options)?)
        .with_fixed_local_offset_minutes(binding_fixed_local_offset_minutes(options)?)
        .with_parse_options(parse)
        .with_ascii_options(ascii_options_from_json(options)?);
    if let Some(site_config) = binding_site_config(options)? {
        renderer = renderer.with_site_config(site_config);
    }

    Ok(renderer)
}

fn ascii_options_from_json(
    options: &crate::common::BindingOptions,
) -> Result<merman::ascii::AsciiRenderOptions, BindingError> {
    let Some(ascii) = options.ascii.as_ref() else {
        return Ok(merman::ascii::AsciiRenderOptions::unicode());
    };

    let mut render_options = merman::ascii::AsciiRenderOptions::unicode();
    if let Some(charset) = ascii.charset.as_deref() {
        render_options.charset = ascii_charset(charset)?;
    }
    if let Some(direction) = ascii.default_direction.as_deref() {
        render_options.default_direction = ascii_direction(direction)?;
    }
    if let Some(color_mode) = ascii.color_mode.as_deref() {
        render_options.color_mode = ascii_color_mode(color_mode)?;
    }
    if let Some(theme) = ascii_theme(ascii)? {
        render_options.color_theme = theme;
    }
    if let Some(sequence_mirror_actors) = ascii.sequence_mirror_actors {
        render_options.sequence_mirror_actors = sequence_mirror_actors;
    }
    if let Some(height) = ascii.xychart_vertical_plot_height {
        render_options.xychart_vertical_plot_height = height;
    }
    if let Some(width) = ascii.xychart_category_band_width {
        render_options.xychart_category_band_width = width;
    }
    if let Some(width) = ascii.xychart_horizontal_plot_width {
        render_options.xychart_horizontal_plot_width = width;
    }
    if let Some(max_grid_cells) = ascii.max_grid_cells {
        render_options.max_grid_cells = max_grid_cells;
    }
    if let Some(relation_summary_diagnostics) = ascii.relation_summary_diagnostics {
        render_options.relation_summary_diagnostics = relation_summary_diagnostics;
    }
    render_options.validate().map_err(|err| {
        BindingError::new(
            BindingStatus::InvalidArgument,
            format!("invalid ascii options: {err}"),
        )
    })?;
    Ok(render_options)
}

fn ascii_theme(
    ascii: &crate::common::AsciiOptionsJson,
) -> Result<Option<merman::ascii::AsciiColorTheme>, BindingError> {
    let Some(theme) = ascii.theme.as_ref() else {
        return Ok(None);
    };

    let foreground = required_ascii_color(theme.foreground.as_deref(), "ascii.theme.foreground")?;
    let background = required_ascii_color(theme.background.as_deref(), "ascii.theme.background")?;
    let mut palette = merman::ascii::AsciiTerminalPalette::new(foreground, background);

    if let Some(line) = optional_ascii_color(theme.line.as_deref(), "ascii.theme.line")? {
        palette = palette.with_line(line);
    }
    if let Some(accent) = optional_ascii_color(theme.accent.as_deref(), "ascii.theme.accent")? {
        palette = palette.with_accent(accent);
    }
    if let Some(muted) = optional_ascii_color(theme.muted.as_deref(), "ascii.theme.muted")? {
        palette = palette.with_muted(muted);
    }
    if let Some(surface) = optional_ascii_color(theme.surface.as_deref(), "ascii.theme.surface")? {
        palette = palette.with_surface(surface);
    }
    if let Some(border) = optional_ascii_color(theme.border.as_deref(), "ascii.theme.border")? {
        palette = palette.with_border(border);
    }

    Ok(Some(merman::ascii::AsciiColorTheme::from_terminal_palette(
        palette,
    )))
}

fn required_ascii_color(
    value: Option<&str>,
    field: &'static str,
) -> Result<merman::ascii::AsciiRgb, BindingError> {
    let Some(value) = value else {
        return Err(invalid_ascii_option(
            field,
            "is required when ascii.theme is provided",
        ));
    };
    optional_ascii_color(Some(value), field)?.ok_or_else(|| {
        invalid_ascii_option(
            field,
            "must be an opaque CSS color representable in terminal output",
        )
    })
}

fn optional_ascii_color(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<merman::ascii::AsciiRgb>, BindingError> {
    let Some(value) = value else {
        return Ok(None);
    };
    merman::ascii::AsciiRgb::parse_css(value)
        .map(Some)
        .ok_or_else(|| {
            invalid_ascii_option(
                field,
                "must be an opaque CSS color representable in terminal output",
            )
        })
}

fn ascii_charset(value: &str) -> Result<merman::ascii::AsciiCharset, BindingError> {
    match option_key(value).as_str() {
        "ascii" => Ok(merman::ascii::AsciiCharset::Ascii),
        "unicode" => Ok(merman::ascii::AsciiCharset::Unicode),
        _ => Err(invalid_ascii_option(
            "ascii.charset",
            "expected `ascii` or `unicode`",
        )),
    }
}

fn ascii_direction(value: &str) -> Result<merman::ascii::AsciiDirection, BindingError> {
    match option_key(value).as_str() {
        "lr" | "left-right" | "left_right" => Ok(merman::ascii::AsciiDirection::LeftRight),
        "td" | "tb" | "top-down" | "top_down" => Ok(merman::ascii::AsciiDirection::TopDown),
        _ => Err(invalid_ascii_option(
            "ascii.default_direction",
            "expected `lr`, `left-right`, `td`, or `top-down`",
        )),
    }
}

fn ascii_color_mode(value: &str) -> Result<merman::ascii::AsciiColorMode, BindingError> {
    match option_key(value).as_str() {
        "plain" | "none" => Ok(merman::ascii::AsciiColorMode::Plain),
        "auto" => Ok(merman::ascii::AsciiColorMode::Auto),
        "ansi16" | "ansi-16" | "ansi_16" => Ok(merman::ascii::AsciiColorMode::Ansi16),
        "ansi256" | "ansi-256" | "ansi_256" => Ok(merman::ascii::AsciiColorMode::Ansi256),
        "truecolor" | "true-color" | "true_color" => Ok(merman::ascii::AsciiColorMode::TrueColor),
        "html" => Ok(merman::ascii::AsciiColorMode::Html),
        _ => Err(invalid_ascii_option(
            "ascii.color_mode",
            "expected `plain`, `auto`, `ansi16`, `ansi256`, `truecolor`, or `html`",
        )),
    }
}

fn option_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn invalid_ascii_option(field: &'static str, message: &'static str) -> BindingError {
    BindingError::new(BindingStatus::InvalidArgument, format!("{field} {message}"))
}

fn render_ascii_with_renderer(
    renderer: &merman::ascii::HeadlessAsciiRenderer,
    source: &str,
) -> Result<Vec<u8>, BindingError> {
    let rendered = renderer
        .render_ascii_sync(source)
        .map_err(classify_ascii_error)?
        .ok_or_else(no_diagram_error)?;

    Ok(rendered.into_bytes())
}

fn classify_ascii_error(err: merman::ascii::HeadlessAsciiError) -> BindingError {
    match err {
        merman::ascii::HeadlessAsciiError::Parse(err) => {
            BindingError::new(BindingStatus::ParseError, err.to_string())
        }
        merman::ascii::HeadlessAsciiError::Ascii(err) => match err {
            merman::ascii::AsciiError::InvalidOption { .. } => {
                BindingError::new(BindingStatus::InvalidArgument, err.to_string())
            }
            merman::ascii::AsciiError::UnsupportedDiagram { .. }
            | merman::ascii::AsciiError::UnsupportedFeature { .. } => {
                BindingError::new(BindingStatus::UnsupportedFormat, err.to_string())
            }
            merman::ascii::AsciiError::RenderLimitExceeded { .. } => {
                BindingError::new(BindingStatus::RenderError, err.to_string())
            }
            _ => BindingError::new(BindingStatus::RenderError, err.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_ascii_returns_unicode_text() {
        let text =
            String::from_utf8(render_ascii(b"flowchart TD\nA[Hello] --> B[World]", b"").unwrap())
                .unwrap();

        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn shared_parse_options_are_stored_under_analysis_options() {
        let options =
            crate::common::parse_options(br#"{ "parse": { "suppress_errors": true } }"#).unwrap();

        assert_eq!(
            options
                .analysis
                .parse
                .as_ref()
                .and_then(|parse| parse.suppress_errors),
            Some(true)
        );
    }

    #[test]
    fn render_ascii_accepts_ascii_options_block() {
        let text = String::from_utf8(
            render_ascii(
                b"flowchart TD\nA[Hello] --> B[World]",
                br#"{ "ascii": { "charset": "ascii" } }"#,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(text.contains("+"), "{text}");
        assert!(text.contains("Hello"));
        assert!(!text.contains("┌"), "{text}");
    }

    #[test]
    fn render_ascii_accepts_camel_case_ascii_options() {
        let text = String::from_utf8(
            render_ascii(
                b"sequenceDiagram\nparticipant A\nparticipant B\nA->>B: Hello",
                br#"{ "ascii": { "sequenceMirrorActors": true } }"#,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(
            text.contains("┌─┴─┐     ┌─┴─┐"),
            "expected mirrored bottom participant boxes:\n{text}"
        );
    }

    #[test]
    fn render_ascii_accepts_relation_summary_diagnostics_option() {
        let text = String::from_utf8(
            render_ascii(
                b"classDiagram\nclass Gateway\nclass Service\nclass Repo\nGateway --> Service : routes\nService --> Repo : stores",
                br#"{ "ascii": { "charset": "ascii", "maxGridCells": 1, "relationSummaryDiagnostics": true } }"#,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(text.contains("relations:"), "{text}");
        assert!(text.contains("reason: grid_budget"), "{text}");
        assert!(text.contains("actual="), "{text}");
        assert!(text.contains("limit=1"), "{text}");
    }

    #[test]
    fn render_ascii_accepts_terminal_palette_theme_options() {
        let text = String::from_utf8(
            render_ascii(
                b"flowchart LR\nA -- yes --> B",
                br##"{ "ascii": { "color_mode": "truecolor", "theme": { "foreground": "#010101", "background": "#ffffff", "line": "#020202", "accent": "#030303", "border": "#040404" } } }"##,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(text.contains("\u{1b}[38;2;1;1;1m"), "{text:?}");
        assert!(text.contains("\u{1b}[38;2;2;2;2m"), "{text:?}");
        assert!(text.contains("\u{1b}[38;2;3;3;3m"), "{text:?}");
        assert!(text.contains("\u{1b}[38;2;4;4;4m"), "{text:?}");
    }

    #[test]
    fn render_ascii_rejects_invalid_terminal_palette_colors() {
        let err = render_ascii(
            b"flowchart TD\nA[Hello]",
            br##"{ "ascii": { "theme": { "foreground": "transparent", "background": "#fff" } } }"##,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("ascii.theme.foreground"), "{err:?}");
    }

    #[test]
    fn render_ascii_rejects_invalid_ascii_option_values() {
        let err = render_ascii(
            b"flowchart TD\nA[Hello]",
            br#"{ "ascii": { "charset": "boxy" } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("ascii.charset"), "{err:?}");
    }

    #[test]
    fn render_ascii_rejects_invalid_ascii_numeric_options() {
        let err = render_ascii(
            b"xychart\nx-axis [A]\ny-axis 0 --> 1\nbar [1]",
            br#"{ "ascii": { "xychart_vertical_plot_height": 1 } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(
            err.message().contains("xychart_vertical_plot_height"),
            "{err:?}"
        );
    }

    #[test]
    fn render_ascii_rejects_invalid_fixed_time_options() {
        let err = render_ascii(
            b"flowchart TD\nA[Hello]",
            br#"{ "fixed_today": "2026/02/15" }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("fixed_today"), "{err:?}");
    }
}
