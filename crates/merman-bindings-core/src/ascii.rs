use crate::common::{
    BindingError, BindingResourceLimitCause, BindingStatus, binding_ascii_resource_policy,
    binding_input_resource_policy, binding_site_config, no_diagram_error, source_text,
};

pub fn render_ascii(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    crate::execute_once_data("ascii", source, None, options_json)
}

#[derive(Clone)]
pub(crate) struct CachedAsciiEngine {
    renderer: merman::Renderer,
    request: merman::AsciiRequest,
    resource_profile: merman::resources::ResourceProfile,
}

pub(crate) struct AsciiOperationConfig {
    runtime_policy: merman::runtime::RuntimePolicy,
    parse_options: merman::ParseOptions,
    render_options: merman::ascii::AsciiRenderOptions,
    ascii_resources: merman::ascii::AsciiResourcePolicy,
    resources: merman::resources::InputResourcePolicy,
    site_config: Option<merman::MermaidConfig>,
}

impl CachedAsciiEngine {
    pub(crate) fn render_ascii(
        &self,
        source: &[u8],
        control: merman::OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        let source = source_text(source)?;
        let output = self
            .renderer
            .render(merman::RenderRequest::ascii(
                source,
                control,
                self.request.clone(),
            ))
            .map_err(|error| classify_render_error(error, self.resource_profile))?;
        let merman::RenderOutput::Ascii(rendered) = output else {
            return Err(BindingError::internal(
                "canonical renderer returned the wrong output variant for `ascii`",
            ));
        };
        rendered
            .map(String::into_bytes)
            .ok_or_else(no_diagram_error)
    }
}

impl AsciiOperationConfig {
    pub(crate) fn compile(
        options: &crate::common::BindingOptions,
        runtime_policy: merman::runtime::RuntimePolicy,
    ) -> Result<Self, BindingError> {
        let parse_options = if options
            .parse
            .as_ref()
            .and_then(|parse| parse.suppress_errors)
            .unwrap_or(false)
        {
            merman::ParseOptions::lenient()
        } else {
            merman::ParseOptions::strict()
        };
        let render_options = ascii_options_from_json(options)?;
        let ascii_resources = binding_ascii_resource_policy(options.analysis.resources.as_ref())?;
        let resources = binding_input_resource_policy(options.analysis.resources.as_ref())?;
        let site_config = binding_site_config(options)?;
        Ok(Self {
            runtime_policy,
            parse_options,
            render_options,
            ascii_resources,
            resources,
            site_config,
        })
    }

    pub(crate) fn materialize(self) -> CachedAsciiEngine {
        let resource_profile = self.resources.profile();
        let mut request = merman::AsciiRequest::default();
        request.options = self.render_options;
        request.resources = self.ascii_resources;
        let mut engine = merman::Engine::new().with_runtime_policy(self.runtime_policy);
        if let Some(site_config) = self.site_config {
            engine = engine.with_site_config(site_config);
        }
        let renderer = merman::Renderer::new()
            .with_engine(engine)
            .with_parse_options(self.parse_options)
            .with_resource_policy(self.resources);
        CachedAsciiEngine {
            renderer,
            request,
            resource_profile,
        }
    }
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
    if let Some(padding) = ascii.box_border_padding {
        render_options.box_border_padding = padding;
    }
    if let Some(padding) = ascii.graph_padding_x {
        render_options.graph_padding_x = padding;
    }
    if let Some(padding) = ascii.graph_padding_y {
        render_options.graph_padding_y = padding;
    }
    if let Some(spacing) = ascii.sequence_participant_spacing {
        render_options.sequence_participant_spacing = spacing;
    }
    if let Some(spacing) = ascii.sequence_message_spacing {
        render_options.sequence_message_spacing = spacing;
    }
    if let Some(width) = ascii.sequence_self_message_width {
        render_options.sequence_self_message_width = width;
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
        "lr" | "leftright" | "left-right" | "left_right" => {
            Ok(merman::ascii::AsciiDirection::LeftRight)
        }
        "td" | "tb" | "topdown" | "top-down" | "top_down" => {
            Ok(merman::ascii::AsciiDirection::TopDown)
        }
        _ => Err(invalid_ascii_option(
            "ascii.default_direction",
            "expected `lr`, `leftRight`, `left-right`, `td`, `topDown`, or `top-down`",
        )),
    }
}

fn ascii_color_mode(value: &str) -> Result<merman::ascii::AsciiColorMode, BindingError> {
    match option_key(value).as_str() {
        "plain" | "none" => Ok(merman::ascii::AsciiColorMode::Plain),
        "ansi16" | "ansi-16" | "ansi_16" => Ok(merman::ascii::AsciiColorMode::Ansi16),
        "ansi256" | "ansi-256" | "ansi_256" => Ok(merman::ascii::AsciiColorMode::Ansi256),
        "truecolor" | "true-color" | "true_color" => Ok(merman::ascii::AsciiColorMode::TrueColor),
        "html" => Ok(merman::ascii::AsciiColorMode::Html),
        _ => Err(invalid_ascii_option(
            "ascii.color_mode",
            "expected `plain`, `ansi16`, `ansi256`, `truecolor`, or `html`",
        )),
    }
}

fn option_key(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn invalid_ascii_option(field: &'static str, message: &'static str) -> BindingError {
    BindingError::new(BindingStatus::InvalidArgument, format!("{field} {message}"))
}

fn classify_render_error(
    err: merman::RenderError,
    resource_profile: merman::resources::ResourceProfile,
) -> BindingError {
    match err {
        merman::RenderError::Cancelled(err) => BindingError::cancelled(err),
        merman::RenderError::RuntimePolicy(err) => crate::common::runtime_policy_error(err),
        merman::RenderError::Parse(err) => {
            BindingError::new(BindingStatus::ParseError, err.to_string())
        }
        merman::RenderError::ResourceLimitExceeded(err) => BindingError::resource_limit_with_cause(
            match err.cause {
                merman::render::ResourceLimitCause::Ceiling => BindingResourceLimitCause::Ceiling,
                merman::render::ResourceLimitCause::ArithmeticOverflow => {
                    BindingResourceLimitCause::ArithmeticOverflow
                }
            },
            err.phase,
            err.id,
            err.actual,
            err.maximum,
            resource_profile.id(),
            err.to_string(),
        ),
        merman::RenderError::Ascii(err) => match err {
            merman::ascii::AsciiError::InvalidOption { .. } => {
                BindingError::new(BindingStatus::InvalidArgument, err.to_string())
            }
            merman::ascii::AsciiError::UnsupportedDiagram { .. }
            | merman::ascii::AsciiError::UnsupportedFeature { .. } => {
                BindingError::new(BindingStatus::UnsupportedOperation, err.to_string())
            }
            _ => BindingError::new(BindingStatus::RenderError, err.to_string()),
        },
        merman::RenderError::UnsupportedTarget(target) => BindingError::internal(format!(
            "renderer returned unsupported target `{target}` for an admitted ASCII operation"
        )),
        _ => BindingError::internal("unknown canonical ASCII renderer failure"),
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
    fn shared_parse_options_are_stored_as_operation_options() {
        let options =
            crate::common::parse_options(br#"{ "parse": { "suppress_errors": true } }"#).unwrap();

        assert_eq!(
            options
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
    fn ascii_layout_options_compile_through_the_public_json_shape() {
        let options = crate::common::parse_options(
            br#"{
                "ascii": {
                    "boxBorderPadding": 2,
                    "graph_padding_x": 3,
                    "graphPaddingY": 4,
                    "sequence_participant_spacing": 6,
                    "sequenceMessageSpacing": 7,
                    "sequence_self_message_width": 8
                }
            }"#,
        )
        .expect("valid ASCII layout options");

        let compiled = ascii_options_from_json(&options).expect("ASCII options compile");
        assert_eq!(compiled.box_border_padding, 2);
        assert_eq!(compiled.graph_padding_x, 3);
        assert_eq!(compiled.graph_padding_y, 4);
        assert_eq!(compiled.sequence_participant_spacing, 6);
        assert_eq!(compiled.sequence_message_spacing, 7);
        assert_eq!(compiled.sequence_self_message_width, 8);
    }

    #[test]
    fn render_ascii_accepts_camel_case_default_direction_values() {
        let text = String::from_utf8(
            render_ascii(
                b"flowchart\nA[Hello] --> B[World]",
                br#"{ "ascii": { "defaultDirection": "topDown" } }"#,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    fn render_ascii_accepts_relation_summary_diagnostics_option() {
        let text = String::from_utf8(
            render_ascii(
                b"classDiagram\nclass A\nclass B\nclass C\nA --> B : ab\nB --> A : ba\nA --> C : ac\nC --> A : ca\nB --> C : bc\nC --> B : cb",
                br#"{ "resources": { "limits": { "max_ascii_grid_cells": 10000 } }, "ascii": { "charset": "ascii", "relationSummaryDiagnostics": true } }"#,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(text.contains("relations:"), "{text}");
        assert!(text.contains("reason: crossing"), "{text}");
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
    fn render_ascii_rejects_environment_dependent_auto_color_mode() {
        let err = render_ascii(
            b"flowchart TD\nA[Hello]",
            br#"{ "ascii": { "color_mode": "auto" } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("ascii.color_mode"), "{err:?}");
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
    fn render_ascii_rejects_invalid_sequence_self_message_width() {
        let err = render_ascii(
            b"sequenceDiagram\nA->>A: Hello",
            br#"{ "ascii": { "sequenceSelfMessageWidth": 1 } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(
            err.message().contains("sequence_self_message_width"),
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

    #[test]
    fn render_ascii_enforces_shared_source_limit_before_parsing() {
        let error = render_ascii(
            b"flowchart TD\nA --> B",
            br#"{ "resources": { "profile": "constrained", "limits": { "max_source_bytes": 4 } } }"#,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        assert!(error.message().contains("max_source_bytes"), "{error:?}");
        let details = error
            .resource_details()
            .expect("structured resource details");
        assert_eq!(details.limit_id, "max_source_bytes");
        assert_eq!(details.phase, "source");
        assert_eq!(details.max, 4);
        assert_eq!(details.profile, "constrained");
    }

    #[test]
    fn render_ascii_enforces_shared_model_cardinality_limit() {
        let error = render_ascii(
            b"flowchart TD\nA --> B",
            br#"{ "resources": { "profile": "constrained", "limits": { "max_model_items": 1 } } }"#,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        assert!(error.message().contains("max_model_items"), "{error:?}");
        let details = error
            .resource_details()
            .expect("structured resource details");
        assert_eq!(details.limit_id, "max_model_items");
        assert_eq!(details.phase, "layout_model");
        assert_eq!(details.max, 1);
        assert_eq!(details.profile, "constrained");
    }

    #[test]
    fn ascii_resource_options_reject_svg_limits() {
        let error = render_ascii(
            b"flowchart TD\nA --> B",
            br#"{ "resources": { "profile": "constrained", "limits": { "max_svg_bytes": 1 } } }"#,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("max_svg_bytes"), "{error:?}");
    }

    #[test]
    fn render_ascii_grid_limit_uses_resource_limit_status() {
        let error = render_ascii(
            b"flowchart TD\nA[Hello] --> B[World]",
            br#"{ "resources": { "limits": { "max_ascii_grid_cells": 1 } } }"#,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        assert!(
            error.message().contains("max_ascii_grid_cells"),
            "{error:?}"
        );
        let details = error
            .resource_details()
            .expect("structured resource details");
        assert_eq!(details.limit_id, "max_ascii_grid_cells");
        assert_eq!(details.phase, "ascii_layout");
        assert_eq!(details.max, 1);
        assert_eq!(details.profile, "interactive");
    }

    #[test]
    fn render_ascii_rejects_removed_grid_limit_field() {
        let error = render_ascii(
            b"flowchart TD\nA --> B",
            br#"{ "ascii": { "maxGridCells": 1 } }"#,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(
            error.message().contains("max_ascii_grid_cells"),
            "{error:?}"
        );
    }
}
