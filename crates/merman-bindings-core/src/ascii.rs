use crate::common::{
    BindingError, BindingResourceLimitCause, BindingStatus, binding_ascii_resource_policy,
    binding_diagnostic_details, binding_input_resource_policy, binding_site_config,
    no_diagram_error, runtime_policy_error, source_text,
};

pub fn render_ascii(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    crate::execute_once_data("ascii", source, None, options_json)
}

#[derive(Clone)]
pub(crate) struct CachedAsciiEngine {
    renderer: merman::Renderer,
    request: merman::AsciiRequest,
    theme: Option<merman::theme::ThemeSelection>,
    resource_profile: merman::model::resources::ResourceProfile,
}

pub(crate) struct AsciiOperationConfig {
    runtime_policy: merman::model::runtime::RuntimePolicy,
    parse_options: merman::ParseOptions,
    render_options: merman::ascii::AsciiRenderOptions,
    viewport: merman::ascii::AsciiViewportPolicy,
    ascii_resources: merman::ascii::AsciiResourcePolicy,
    resources: merman::model::resources::InputResourcePolicy,
    site_config: Option<merman::MermaidConfig>,
    theme: Option<merman::theme::ThemeSelection>,
}

impl CachedAsciiEngine {
    pub(crate) fn render_ascii_output(
        &self,
        source: &[u8],
        control: merman::OperationControl,
    ) -> Result<merman::ascii::AsciiOutput, BindingError> {
        let source = source_text(source)?;
        let mut request = merman::RenderRequest::ascii(source, control, self.request.clone());
        if let Some(theme) = &self.theme {
            request = request.with_native_theme(theme.clone());
        }
        let output = self
            .renderer
            .render(request)
            .map_err(|error| classify_render_error(error, self.resource_profile))?;
        let merman::RenderOutput::Ascii(rendered) = output else {
            return Err(BindingError::internal(
                "canonical renderer returned the wrong output variant for `ascii`",
            ));
        };
        rendered.ok_or_else(no_diagram_error)
    }
}

impl AsciiOperationConfig {
    pub(crate) fn compile(
        options: &crate::common::BindingOptions,
        runtime_policy: merman::model::runtime::RuntimePolicy,
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
        let viewport = ascii_viewport_from_json(options)?;
        let ascii_resources = binding_ascii_resource_policy(options.analysis.resources.as_ref())?;
        let resources = binding_input_resource_policy(options.analysis.resources.as_ref())?;
        let site_config = binding_site_config(options)?;
        let has_explicit_palette = options
            .ascii
            .as_ref()
            .is_some_and(|ascii| ascii.theme.is_some());
        let theme = if has_explicit_palette {
            None
        } else {
            options.theme.clone()
        };
        Ok(Self {
            runtime_policy,
            parse_options,
            render_options,
            viewport,
            ascii_resources,
            resources,
            site_config,
            theme,
        })
    }

    pub(crate) fn materialize(self) -> CachedAsciiEngine {
        let resource_profile = self.resources.profile();
        let request = merman::AsciiRequest {
            options: self.render_options,
            resources: self.ascii_resources,
            viewport: self.viewport,
            native_theme: None,
        };
        let mut parser = merman::SemanticParser::new().with_runtime_policy(self.runtime_policy);
        if let Some(site_config) = self.site_config {
            parser = parser.with_site_config(site_config);
        }
        let renderer = merman::Renderer::new()
            .with_semantic_parser(parser)
            .with_parse_options(self.parse_options)
            .with_resource_policy(self.resources);
        CachedAsciiEngine {
            renderer,
            request,
            theme: self.theme,
            resource_profile,
        }
    }
}

fn ascii_viewport_from_json(
    options: &crate::common::BindingOptions,
) -> Result<merman::ascii::AsciiViewportPolicy, BindingError> {
    let Some(ascii) = options.ascii.as_ref() else {
        return Ok(merman::ascii::AsciiViewportPolicy::default());
    };
    let overflow = match ascii.overflow.as_deref().map(option_key).as_deref() {
        None | Some("allow") => merman::ascii::OverflowPolicy::Allow,
        Some("fallback") => merman::ascii::OverflowPolicy::Fallback,
        Some("error") => merman::ascii::OverflowPolicy::Error,
        Some(_) => {
            return Err(invalid_ascii_option(
                "ascii.overflow",
                "expected `allow`, `fallback`, or `error`",
            ));
        }
    };
    let trim = if ascii.trim_trailing_spaces.unwrap_or(false) {
        merman::ascii::AsciiTrimPolicy::TrimTrailingSpaces
    } else {
        merman::ascii::AsciiTrimPolicy::Preserve
    };
    let mut viewport = merman::ascii::AsciiViewportPolicy::default()
        .overflow(overflow)
        .trim(trim);
    if let Some(max_width) = ascii.max_width {
        if (max_width as u128) > u128::from(crate::RUNTIME_CATALOG_MAX_SAFE_INTEGER) {
            return Err(invalid_ascii_option(
                "ascii.max_width",
                "must not exceed the portable JSON integer limit",
            ));
        }
        viewport = viewport.max_width(max_width);
    }
    viewport.validate().map_err(|error| {
        BindingError::new(
            BindingStatus::InvalidArgument,
            format!("invalid ascii viewport: {error}"),
        )
    })?;
    Ok(viewport)
}

fn ascii_options_from_json(
    options: &crate::common::BindingOptions,
) -> Result<merman::ascii::AsciiRenderOptions, BindingError> {
    let Some(ascii) = options.ascii.as_ref() else {
        return Ok(merman::ascii::AsciiRenderOptions::unicode());
    };

    let mut render_options = merman::ascii::AsciiRenderOptions::unicode();
    if let Some(profile) = ascii.layout_profile.as_deref() {
        render_options.layout_profile = match option_key(profile).as_str() {
            "canonical" => merman::ascii::AsciiLayoutProfile::Canonical,
            "compact" => merman::ascii::AsciiLayoutProfile::Compact,
            "auto" => merman::ascii::AsciiLayoutProfile::Auto,
            _ => {
                return Err(invalid_ascii_option(
                    "ascii.layout_profile",
                    "expected `canonical`, `compact`, or `auto`",
                ));
            }
        };
    }
    if let Some(presentation) = ascii.graph_presentation.as_deref() {
        render_options.graph_presentation = match option_key(presentation).as_str() {
            "strict" => merman::ascii::AsciiGraphPresentation::Strict,
            "adaptive" => merman::ascii::AsciiGraphPresentation::Adaptive,
            _ => {
                return Err(invalid_ascii_option(
                    "ascii.graph_presentation",
                    "expected `strict` or `adaptive`",
                ));
            }
        };
    }
    if let Some(space) = ascii.graph_routing_space.as_deref() {
        render_options.graph_routing_space = match option_key(space).as_str() {
            "expand" => merman::ascii::AsciiGraphRoutingSpace::Expand,
            "fixed" => merman::ascii::AsciiGraphRoutingSpace::Fixed,
            _ => {
                return Err(invalid_ascii_option(
                    "ascii.graph_routing_space",
                    "expected `expand` or `fixed`",
                ));
            }
        };
    }
    if let Some(charset) = ascii.charset.as_deref() {
        render_options.charset = ascii_charset(charset)?;
    }
    if let Some(profile) = ascii.width_profile.as_deref() {
        render_options.terminal_width_profile = ascii_width_profile(profile)?;
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
        render_options = render_options.with_node_padding(padding);
    }
    if let Some(padding) = ascii.node_padding_x {
        render_options = render_options.with_node_padding_x(padding);
    }
    if let Some(padding) = ascii.node_padding_y {
        render_options = render_options.with_node_padding_y(padding);
    }
    if let Some(padding) = ascii.graph_padding_x {
        render_options = render_options.with_graph_padding_x(padding);
    }
    if let Some(padding) = ascii.graph_padding_y {
        render_options = render_options.with_graph_padding_y(padding);
    }
    if let Some(width) = ascii.flowchart_node_label_wrap_width {
        render_options = render_options.with_flowchart_node_label_wrap_width(width);
    }
    if let Some(spacing) = ascii.sequence_participant_spacing {
        render_options = render_options.with_sequence_participant_spacing(spacing);
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

fn ascii_width_profile(value: &str) -> Result<merman::ascii::TerminalWidthProfile, BindingError> {
    match option_key(value).as_str() {
        "unicode" => Ok(merman::ascii::TerminalWidthProfile::Unicode),
        "cjk" => Ok(merman::ascii::TerminalWidthProfile::Cjk),
        _ => Err(invalid_ascii_option(
            "ascii.width_profile",
            "expected `unicode` or `cjk`",
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
    resource_profile: merman::model::resources::ResourceProfile,
) -> BindingError {
    match err {
        merman::RenderError::NoDiagram => no_diagram_error(),
        merman::RenderError::Cancelled(err) => BindingError::cancelled(err),
        merman::RenderError::RuntimePolicy(err) => runtime_policy_error(err),
        merman::RenderError::Parse(err) => crate::common::parse_error(err),
        merman::RenderError::Theme(err) => crate::native_theme::compile_error("theme", err),
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
            merman::normalize_terminal_diagnostic(&err.to_string()),
        ),
        merman::RenderError::Ascii(err) => {
            let status = match &err {
                merman::ascii::AsciiError::InvalidOption { .. } => BindingStatus::InvalidArgument,
                merman::ascii::AsciiError::UnsupportedDiagram { .. }
                | merman::ascii::AsciiError::UnsupportedFeature { .. } => {
                    BindingStatus::UnsupportedOperation
                }
                merman::ascii::AsciiError::RoutingSearchExhausted { .. }
                | merman::ascii::AsciiError::PortCapacityExceeded { .. } => {
                    BindingStatus::RenderError
                }
                _ => BindingStatus::RenderError,
            };
            let error = merman::ascii::AsciiDiagnostic::from(err);
            let safe_message = error.terminal_safe_message();
            let diagnostic = error
                .terminal_diagnostic_details()
                .map(binding_diagnostic_details);
            match diagnostic {
                Some(diagnostic) => {
                    BindingError::new(status, safe_message).with_diagnostic_details(diagnostic)
                }
                None => BindingError::new(status, safe_message),
            }
        }
        merman::RenderError::UnsupportedTarget(target) => BindingError::internal(format!(
            "renderer returned unsupported target `{target}` for an admitted ASCII operation"
        )),
        _ => BindingError::internal("unknown canonical ASCII renderer failure"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use merman_ascii_test_contracts::ascii_resource_boundary_contract;

    fn render_ascii_with_limit(
        limit_id: &str,
        max: u64,
        source: &str,
    ) -> std::result::Result<Vec<u8>, BindingError> {
        let options = format!(r#"{{ "resources": {{ "limits": {{ "{limit_id}": {max} }} }} }}"#);
        render_ascii(source.as_bytes(), options.as_bytes())
    }

    fn assert_binding_ascii_exact_boundary(
        limit_id: &str,
        phase: &str,
        expected: u64,
        source: &str,
    ) {
        let output = render_ascii_with_limit(limit_id, expected, source)
            .unwrap_or_else(|error| panic!("exact {limit_id} boundary failed: {error:?}"));
        assert!(!output.is_empty(), "{limit_id} produced no output");
        let error = match render_ascii_with_limit(limit_id, expected - 1, source) {
            Err(error) => error,
            Ok(output) => panic!(
                "one-below {limit_id} boundary must fail, but rendered {} bytes",
                output.len()
            ),
        };
        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        let details = error
            .resource_details()
            .expect("ASCII resource errors expose structured details");
        assert_eq!(details.limit_id, limit_id);
        assert_eq!(details.phase, phase);
        assert_eq!(details.actual, expected);
        assert_eq!(details.max, expected - 1);
        assert_eq!(details.profile, "interactive");
        assert_eq!(details.cause, BindingResourceLimitCause::Ceiling);
    }

    #[test]
    fn theme_evaluation_limits_keep_resource_status_for_ascii_output() {
        let error = classify_render_error(
            merman::RenderError::Parse(
                merman::Error::ThemeEvaluationLimit(merman::ThemeEvaluationLimitExceeded {
                    limit: "THEME_COLOR_LIMIT",
                    requested: "65".to_string(),
                    max: 64,
                })
                .into(),
            ),
            merman::model::resources::ResourceProfile::Interactive,
        );

        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        assert!(error.message().contains("THEME_COLOR_LIMIT"));
        assert_eq!(error.resource_details(), None);
    }

    #[test]
    fn routing_search_exhaustion_preserves_render_status_and_safe_diagnostic_payload() {
        let error = classify_render_error(
            merman::ascii::AsciiError::RoutingSearchExhausted {
                diagram_type: "flowchart\u{1b}\u{7}\u{9b}\u{202e}",
                edge_index: 7,
            }
            .into(),
            merman::model::resources::ResourceProfile::Interactive,
        );

        assert_eq!(error.status(), BindingStatus::RenderError);
        assert_eq!(error.resource_details(), None);
        assert!(error.message().contains("edge[7]"), "{error:?}");
        assert!(
            error.message().contains("did not find a valid scene"),
            "{error:?}"
        );
        for misleading in [
            "unsupported",
            "does not support",
            "impossible",
            "resource limit",
        ] {
            assert!(!error.message().contains(misleading), "{error:?}");
        }
        assert!(!error.message().chars().any(char::is_control));
        assert!(!error.message().contains('\u{202e}'));
        let details = error
            .diagnostic_details()
            .expect("routing search exhaustion should preserve diagnostic details");
        assert_eq!(details.code, "merman.ascii.routing_search_exhausted");
        assert_eq!(details.field.as_deref(), Some("edge[7]"));
        assert_eq!(
            details.diagram_type.as_deref(),
            Some("flowchart\\u{1B}\\u{7}\\u{9B}\\u{202E}")
        );

        let payload: serde_json::Value =
            serde_json::from_slice(&crate::binding_error_payload_json_bytes(&error))
                .expect("binding error payload should be JSON");
        assert_eq!(payload["code_name"], "MERMAN_RENDER_ERROR");
        assert_eq!(payload["message"], error.message());
        assert_eq!(
            payload["details"]["diagnostic"]["code"],
            "merman.ascii.routing_search_exhausted"
        );
        assert_eq!(payload["details"]["diagnostic"]["field"], "edge[7]");
        assert_eq!(
            payload["details"]["diagnostic"]["diagram_type"],
            "flowchart\\u{1B}\\u{7}\\u{9B}\\u{202E}"
        );
    }

    #[test]
    fn port_capacity_preserves_render_status_and_safe_endpoint_payload() {
        let error = classify_render_error(
            merman::ascii::AsciiError::PortCapacityExceeded {
                diagram_type: "flowchart\u{1b}\u{9b}",
                endpoint: "source\u{7}\u{202e}".to_string(),
                required: 33,
                available: 32,
            }
            .into(),
            merman::model::resources::ResourceProfile::Interactive,
        );

        assert_eq!(error.status(), BindingStatus::RenderError);
        assert_eq!(error.resource_details(), None);
        assert!(error.message().contains("current presentation"));
        assert!(
            error
                .message()
                .contains("33 independent markers > upper bound 32")
        );
        assert!(!error.message().chars().any(char::is_control));
        assert!(!error.message().contains('\u{202e}'));
        let details = error.diagnostic_details().unwrap();
        assert_eq!(details.code, "merman.ascii.port_capacity_exceeded");
        assert_eq!(details.field.as_deref(), Some("source\\u{7}\\u{202E}"));
        assert_eq!(
            details.diagram_type.as_deref(),
            Some("flowchart\\u{1B}\\u{9B}")
        );

        let payload: serde_json::Value =
            serde_json::from_slice(&crate::binding_error_payload_json_bytes(&error)).unwrap();
        assert_eq!(payload["code_name"], "MERMAN_RENDER_ERROR");
        assert_eq!(payload["message"], error.message());
        assert_eq!(
            payload["details"]["diagnostic"]["code"],
            "merman.ascii.port_capacity_exceeded"
        );
        assert_eq!(
            payload["details"]["diagnostic"]["field"],
            "source\\u{7}\\u{202E}"
        );
        assert_eq!(
            payload["details"]["diagnostic"]["diagram_type"],
            "flowchart\\u{1B}\\u{9B}"
        );
    }

    #[test]
    fn render_ascii_returns_unicode_text() {
        let text =
            String::from_utf8(render_ascii(b"flowchart TD\nA[Hello] --> B[World]", b"").unwrap())
                .unwrap();

        assert!(text.contains("Hello"));
        assert!(text.contains("World"));
    }

    #[test]
    #[cfg(feature = "diagram-flowchart")]
    fn render_ascii_binding_applies_fixed_routing_space_from_both_json_spellings() {
        for key in ["graph_routing_space", "graphRoutingSpace"] {
            let options = format!(r#"{{"ascii":{{"{key}":"fixed","graphPaddingY":2}}}}"#);
            let bytes = render_ascii(
                b"flowchart TD\nA[Load] -->|submit| B[Save]",
                options.as_bytes(),
            )
            .unwrap();
            let text = String::from_utf8(bytes).unwrap();
            assert_eq!(text.lines().count(), 8, "{text}");
            assert!(text.contains("submit"));
        }
    }

    #[test]
    fn render_ascii_result_exposes_the_canonical_report_plan() {
        let options = br#"{
            "ascii": {
                "maxWidth": 10,
                "overflow": "fallback",
                "trimTrailingSpaces": true
            }
        }"#;
        let result = crate::render_ascii_result(
            b"timeline\ntitle Basic\n2024 : Event with a long authored value",
            options,
        )
        .expect("timeline fallback should produce a report-bearing result");
        let Some(crate::BindingOutputPlan::Ascii(plan)) = result.metadata().output_plan() else {
            panic!("ASCII operation must expose its canonical output plan");
        };

        assert_eq!(
            result.data(),
            crate::render_ascii(
                b"timeline\ntitle Basic\n2024 : Event with a long authored value",
                options,
            )
            .unwrap()
        );
        assert_eq!(plan.family(), "timeline");
        assert_eq!(plan.projection(), "structured_text");
        assert_eq!(plan.schema_version(), 4);
        assert_eq!(plan.encoding(), "plain");
        assert_eq!(plan.outcome(), "fallback");
        assert_eq!(plan.requested_max_width(), Some(10));
        assert_eq!(plan.requested_graph_presentation(), "strict");
        assert_eq!(plan.primary_overflowed(), Some(true));
        assert!(!plan.emitted_overflowed());
        assert!(plan.fallback_attempted());

        let metadata: serde_json::Value =
            serde_json::from_slice(result.metadata_json()).expect("metadata JSON");
        assert_eq!(metadata["output_plan"]["kind"], "ascii");
        assert_eq!(metadata["output_plan"]["encoding"], "plain");
        assert_eq!(
            metadata["output_plan"]["emitted_width"],
            plan.emitted_width()
        );
    }

    #[test]
    #[cfg(all(
        feature = "diagram-architecture",
        feature = "diagram-usecase",
        feature = "diagram-block",
        feature = "diagram-c4",
        feature = "diagram-agentflow"
    ))]
    fn family_binding_results_expose_the_selected_primary_and_fallback() {
        for (family, source) in [
            ("agentflow", "agentflow-beta\nA[Request] --> B[Result]\n"),
            (
                "architecture",
                "architecture-beta\nservice api(server)[API]\nservice db(database)[Database]\napi:R --> L:db\n",
            ),
            (
                "usecase",
                "usecase-beta\nactor Customer(\"Customer\")\nCheckout(\"Place order\")\nCustomer --> Checkout\n",
            ),
            (
                "block",
                "block-beta\ncolumns 3\nA[\"Request\"]:2 space\nB[\"Result\"]:3\nA --> B\n",
            ),
            (
                "c4",
                "C4Container\nPerson(user, \"Customer\")\nBoundary(zone, \"Services\") {\nContainer(api, \"API\", \"Rust\", \"Serves requests\")\n}\nRel_Back(api, user, \"Replies\", \"HTTPS\")\n",
            ),
        ] {
            for width in [10, 80] {
                let options = format!(
                    r#"{{"ascii":{{"maxWidth":{width},"overflow":"fallback","colorMode":"plain"}}}}"#
                );
                let result =
                    crate::render_ascii_result(source.as_bytes(), options.as_bytes()).unwrap();
                let Some(crate::BindingOutputPlan::Ascii(plan)) = result.metadata().output_plan()
                else {
                    panic!("ASCII result must retain its canonical plan");
                };
                assert_eq!(plan.family(), family);
                assert_eq!(plan.schema_version(), 4);
                assert_eq!(plan.encoding(), "plain");
                assert_eq!(plan.layout_profile(), Some("canonical"));
                assert_eq!(plan.requested_graph_presentation(), "strict");
                assert_eq!(plan.fallback_attempted(), width == 10);
                assert!(!plan.emitted_overflowed());
                assert_eq!(
                    plan.projection(),
                    if width == 10 {
                        "structured_text"
                    } else {
                        "diagrammatic"
                    }
                );
                assert!(plan.emitted_width() <= width);
                let metadata: serde_json::Value =
                    serde_json::from_slice(result.metadata_json()).unwrap();
                assert_eq!(metadata["output_plan"]["family"], family);
                assert_eq!(metadata["output_plan"]["schema_version"], 4);
                if width == 10 {
                    let text = std::str::from_utf8(result.data()).unwrap();
                    let compact: String = text.split_whitespace().collect();
                    let relationships: &[&str] = if family == "usecase" {
                        &[
                            "E1:N1[open]--solidassociation--N2[point]",
                            "id=\"Checkout\"",
                        ]
                    } else if family == "block" {
                        &[
                            "start=\"A\"",
                            "end=\"B\"",
                            "end-marker=\"arrow_point\"",
                            "columns=3",
                            "span=2",
                            "type=\"space\"",
                            "children=[",
                        ]
                    } else if family == "c4" {
                        &[
                            "E1[relationship]type=\"rel_b\"source=[S2]alias=\"api\"target=[S1]alias=\"user\"heads=source",
                            "S2techn={text:\"Rust\"}",
                            "parentBoundary=[B2]alias=\"zone\"",
                        ]
                    } else if family == "architecture" {
                        &[
                            "N1[service]id=\"api\"title=\"API\"",
                            "N2[service]id=\"db\"title=\"Database\"",
                            "E1lhs=[N1]id=\"api\"lhsDir=\"R\"lhsInto=null",
                            "E1rhs=[N2]id=\"db\"rhsDir=\"L\"rhsInto=true",
                        ]
                    } else {
                        &[
                            "E1[sequence-->]source=[N1]id=\"A\"target=[N2]id=\"B\"",
                            "explicit-id=false",
                        ]
                    };
                    for expected in relationships {
                        assert!(compact.contains(expected), "missing {expected:?}: {text}");
                    }
                }
            }
        }
    }

    #[test]
    #[cfg(feature = "diagram-gantt")]
    fn gantt_binding_results_distinguish_timeline_compact_records_and_width_fallback() {
        for (compact_mode, width, width_profile) in [
            (false, 60, "unicode"),
            (false, 59, "cjk"),
            (true, 59, "unicode"),
        ] {
            let prefix = if compact_mode {
                "---\nconfig:\n  gantt:\n    displayMode: compact\n---\n"
            } else {
                ""
            };
            let source = format!(
                "{prefix}gantt\ndateFormat YYYY-MM-DD\nsection Delivery work\nBuild phase: done, build, 2026-01-01, 2d\nReview phase: crit, active, review, after build, 1d\n"
            );
            let options = serde_json::to_vec(&serde_json::json!({
                "ascii": {
                    "maxWidth": width,
                    "widthProfile": width_profile,
                    "layoutProfile": "canonical",
                    "graphPresentation": "strict",
                    "colorMode": "plain",
                    "overflow": "fallback",
                },
            }))
            .unwrap();
            let result = crate::render_ascii_result(source.as_bytes(), &options).unwrap();
            let Some(crate::BindingOutputPlan::Ascii(plan)) = result.metadata().output_plan()
            else {
                panic!("Gantt ASCII result must retain its canonical plan");
            };
            let fallback = !compact_mode && width < 60;
            assert_eq!(
                result.data(),
                render_ascii(source.as_bytes(), &options).unwrap()
            );
            assert_eq!(plan.family(), "gantt");
            assert_eq!(plan.schema_version(), 4);
            assert_eq!(plan.encoding(), "plain");
            assert_eq!(plan.width_profile(), width_profile);
            assert_eq!(plan.layout_profile(), Some("canonical"));
            assert_eq!(plan.requested_graph_presentation(), "strict");
            assert_eq!(plan.requested_max_width(), Some(width));
            assert!(!plan.compact_attempted());
            assert_eq!(plan.fallback_capability(), "available");
            assert_eq!(plan.fallback_attempted(), fallback);
            assert_eq!(plan.primary_overflowed(), Some(fallback));
            assert_eq!(
                plan.fallback_reason(),
                if fallback {
                    Some("primary_overflow")
                } else {
                    None
                }
            );
            assert_eq!(
                plan.outcome(),
                if fallback { "fallback" } else { "primary" }
            );
            assert_eq!(
                plan.projection(),
                if compact_mode || fallback {
                    "structured_text"
                } else {
                    "diagrammatic"
                }
            );
            assert!(!plan.emitted_overflowed());
            assert!(plan.emitted_width() <= width);
            let metadata: serde_json::Value =
                serde_json::from_slice(result.metadata_json()).unwrap();
            assert_eq!(metadata["output_plan"]["family"], "gantt");
            assert_eq!(metadata["output_plan"]["projection"], plan.projection());
            assert_eq!(metadata["output_plan"]["outcome"], plan.outcome());
            assert_eq!(
                metadata["output_plan"]["emitted_width"],
                plan.emitted_width()
            );
            let text = std::str::from_utf8(result.data()).unwrap();
            assert!(!text.contains('\u{1b}'));
            assert_eq!(
                text.contains("Gantt timeline (elapsed time)"),
                !compact_mode && !fallback
            );
            let compact: String = text.split_whitespace().collect();
            for expected in [
                "T1[task]id=\"build\"",
                "T2[task]id=\"review\"",
                "T1details:{",
                "T2details:{",
                "ganttdetails:{",
            ] {
                assert!(compact.contains(expected), "missing {expected:?}: {text}");
            }
        }
    }

    #[test]
    #[cfg(feature = "diagram-gantt")]
    fn gantt_binding_rejects_unadmitted_output_combinations() {
        for ascii in [
            serde_json::json!({"layoutProfile": "compact"}),
            serde_json::json!({"layoutProfile": "auto", "maxWidth": 59}),
            serde_json::json!({"graphPresentation": "adaptive"}),
            serde_json::json!({"colorMode": "ansi16", "maxWidth": 59, "overflow": "fallback"}),
            serde_json::json!({"colorMode": "ansi256", "maxWidth": 59, "overflow": "fallback"}),
            serde_json::json!({"colorMode": "truecolor", "maxWidth": 59, "overflow": "fallback"}),
            serde_json::json!({"colorMode": "html", "maxWidth": 59, "overflow": "fallback"}),
        ] {
            let options = serde_json::to_vec(&serde_json::json!({"ascii": ascii})).unwrap();
            let error = render_ascii(
                b"gantt\ndateFormat YYYY-MM-DD\nBuild: build, 2026-01-01, 2d\n",
                &options,
            )
            .expect_err("Gantt admits only Canonical, Strict, and Plain viewport fallback");
            assert_eq!(error.status(), BindingStatus::InvalidArgument, "{error:?}");
        }
    }

    #[test]
    #[cfg(feature = "diagram-zenuml")]
    fn zenuml_binding_results_keep_the_basic_scene_and_complete_width_fallback() {
        let source = b"zenuml\nAlice->Bob: Hello\nBob-->Alice: Reply\n";
        for width in [None, Some(10)] {
            let mut options = serde_json::json!({
                "ascii": {"colorMode": "plain", "overflow": "fallback"},
            });
            if let Some(width) = width {
                options["ascii"]["maxWidth"] = serde_json::json!(width);
            }
            let options = serde_json::to_vec(&options).unwrap();
            let result = crate::render_ascii_result(source, &options).unwrap();
            let Some(crate::BindingOutputPlan::Ascii(plan)) = result.metadata().output_plan()
            else {
                panic!("ZenUML ASCII result must retain its canonical plan");
            };
            assert_eq!(result.data(), render_ascii(source, &options).unwrap());
            assert_eq!(plan.family(), "zenuml");
            assert_eq!(plan.schema_version(), 4);
            assert_eq!(plan.encoding(), "plain");
            assert_eq!(plan.layout_profile(), Some("canonical"));
            assert_eq!(plan.requested_graph_presentation(), "strict");
            assert_eq!(plan.requested_max_width(), width);
            assert!(!plan.compact_attempted());
            assert_eq!(plan.fallback_capability(), "available");
            assert_eq!(plan.fallback_attempted(), width.is_some());
            assert_eq!(plan.primary_overflowed(), Some(width.is_some()));
            assert_eq!(
                plan.fallback_reason(),
                if width.is_some() {
                    Some("primary_overflow")
                } else {
                    None
                }
            );
            assert_eq!(
                plan.outcome(),
                if width.is_some() {
                    "fallback"
                } else {
                    "primary"
                }
            );
            assert_eq!(
                plan.projection(),
                if width.is_some() {
                    "structured_text"
                } else {
                    "diagrammatic"
                }
            );
            assert!(!plan.emitted_overflowed());
            if let Some(width) = width {
                assert!(plan.emitted_width() <= width);
            }
            let metadata: serde_json::Value =
                serde_json::from_slice(result.metadata_json()).unwrap();
            assert_eq!(metadata["output_plan"]["family"], "zenuml");
            assert_eq!(metadata["output_plan"]["projection"], plan.projection());
            assert_eq!(metadata["output_plan"]["outcome"], plan.outcome());
            assert_eq!(
                metadata["output_plan"]["emitted_width"],
                plan.emitted_width()
            );
            let text = std::str::from_utf8(result.data()).unwrap();
            assert!(!text.contains('\u{1b}'));
            let compact: String = text.split_whitespace().collect();
            for expected in [
                "P1[participant]name=\"Alice\"",
                "P2[participant]name=\"Bob\"",
                "S1[message]id=\"zenuml-statement-0\"parent=nullsection=nullfrom=[P1]name=\"Alice\"to=[P2]name=\"Bob\"style=asynchronousassignment=nulllabel=\"Hello\"bodyCount=0",
                "S2[return]id=\"zenuml-statement-1\"parent=nullsection=nullfrom=[P2]name=\"Bob\"to=[P1]name=\"Alice\"label=\"Reply\"",
                "S1details:{",
                "S2details:{",
                "explicit_from:\"Alice\"",
                "explicit_to:\"Alice\"",
                "span:{",
                "occurrences:[",
            ] {
                assert!(compact.contains(expected), "missing {expected:?}: {text}");
            }
        }
    }

    #[test]
    #[cfg(feature = "diagram-zenuml")]
    fn zenuml_binding_rejects_unadmitted_output_combinations() {
        for ascii in [
            serde_json::json!({"layoutProfile": "compact"}),
            serde_json::json!({"layoutProfile": "auto"}),
            serde_json::json!({"graphPresentation": "adaptive"}),
            serde_json::json!({"colorMode": "ansi16", "maxWidth": 10, "overflow": "fallback"}),
        ] {
            let options = serde_json::to_vec(&serde_json::json!({"ascii": ascii})).unwrap();
            let error = render_ascii(b"zenuml\nAlice->Bob: Hello\n", &options)
                .expect_err("ZenUML admits only Canonical, Strict, and Plain viewport fallback");
            assert_eq!(error.status(), BindingStatus::InvalidArgument, "{error:?}");
        }
    }

    #[test]
    #[cfg(feature = "diagram-pie")]
    fn pie_binding_reports_keep_actual_projection_and_complete_keyed_records() {
        for (dogs, cats, primary_projection) in
            [(386, 85, "diagrammatic"), (0, 0, "structured_text")]
        {
            let source = format!("pie showData\ntitle Pets\n\"Dogs\": {dogs}\n\"Cats\": {cats}\n");
            for width in [10, 20, 80] {
                let options = serde_json::json!({
                    "ascii": {
                        "maxWidth": width,
                        "overflow": "fallback",
                        "colorMode": "plain",
                        "graphPresentation": "strict",
                    },
                });
                let result = crate::render_ascii_result(
                    source.as_bytes(),
                    &serde_json::to_vec(&options).unwrap(),
                )
                .unwrap();
                let Some(crate::BindingOutputPlan::Ascii(plan)) = result.metadata().output_plan()
                else {
                    panic!("Pie ASCII result must retain its canonical plan");
                };
                assert_eq!(plan.family(), "pie");
                assert_eq!(plan.schema_version(), 4);
                assert_eq!(plan.encoding(), "plain");
                assert_eq!(plan.layout_profile(), Some("canonical"));
                assert_eq!(plan.requested_graph_presentation(), "strict");
                assert_eq!(plan.requested_max_width(), Some(width));
                assert!(!plan.compact_attempted());
                assert_eq!(plan.fallback_capability(), "available");
                assert_eq!(plan.fallback_attempted(), width < 80);
                assert_eq!(
                    plan.fallback_reason(),
                    if width < 80 {
                        Some("primary_overflow")
                    } else {
                        None
                    }
                );
                assert_eq!(plan.primary_overflowed(), Some(width < 80));
                assert_eq!(
                    plan.outcome(),
                    if width < 80 { "fallback" } else { "primary" }
                );
                assert_eq!(
                    plan.projection(),
                    if width < 80 {
                        "structured_text"
                    } else {
                        primary_projection
                    }
                );
                assert!(!plan.emitted_overflowed());
                assert!(plan.emitted_width() <= width);
                assert_eq!(
                    plan.lossiness(),
                    if width < 80 && primary_projection == "diagrammatic" {
                        "presentation_only"
                    } else {
                        "none"
                    }
                );
                let metadata: serde_json::Value =
                    serde_json::from_slice(result.metadata_json()).unwrap();
                assert_eq!(metadata["output_plan"]["family"], "pie");
                assert_eq!(metadata["output_plan"]["projection"], plan.projection());
                assert_eq!(metadata["output_plan"]["outcome"], plan.outcome());
                let text = std::str::from_utf8(result.data()).unwrap();
                assert!(!text.contains('\u{1b}'));
                let compact: String = text.split_whitespace().collect();
                for expected in [
                    format!("P1label=\"Dogs\"value={dogs}"),
                    format!("P2label=\"Cats\"value={cats}"),
                    "Pets".to_string(),
                ] {
                    assert!(compact.contains(&expected), "missing {expected:?}: {text}");
                }
                assert!(compact.find("P1label=").unwrap() < compact.find("P2label=").unwrap());
            }
        }

        let error = render_ascii(
            b"pie\ntitle Pets\n\"Dogs\": 386\n\"Cats\": 85\n",
            br#"{"ascii":{"graphPresentation":"adaptive","colorMode":"plain"}}"#,
        )
        .expect_err("Pie admits only Strict graph presentation");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("graph_presentation"), "{error:?}");
    }

    #[test]
    #[cfg(feature = "diagram-requirement")]
    fn requirement_binding_reports_keep_primary_records_and_complete_width_fallback() {
        let source = concat!(
            "requirementDiagram\n",
            "requirement req {\nid: R-1\ntext: Login required\nrisk: high\nverifymethod: test\n}\n",
            "element sys {\ntype: service\ndocref: spec.md\n}\n",
            "sys - satisfies -> req\n",
        );
        for (source, primary_projection) in [
            (source.to_string(), "diagrammatic"),
            (
                source.replace("sys - satisfies", "missing - satisfies"),
                "structured_text",
            ),
        ] {
            for (width, presentation) in [
                (4, "strict"),
                (10, "strict"),
                (80, "strict"),
                (80, "adaptive"),
            ] {
                let options = serde_json::json!({
                    "ascii": {
                        "maxWidth": width,
                        "overflow": "fallback",
                        "colorMode": "plain",
                        "graphPresentation": presentation,
                    },
                });
                let result = crate::render_ascii_result(
                    source.as_bytes(),
                    &serde_json::to_vec(&options).unwrap(),
                )
                .unwrap();
                let Some(crate::BindingOutputPlan::Ascii(plan)) = result.metadata().output_plan()
                else {
                    panic!("Requirement ASCII result must retain its canonical plan");
                };
                assert_eq!(plan.family(), "requirement");
                assert_eq!(plan.schema_version(), 4);
                assert_eq!(plan.encoding(), "plain");
                assert_eq!(plan.layout_profile(), Some("canonical"));
                assert_eq!(plan.requested_graph_presentation(), presentation);
                assert_eq!(plan.requested_max_width(), Some(width));
                let fallback_expected = width == 4 && primary_projection == "diagrammatic";
                assert_eq!(plan.fallback_attempted(), fallback_expected);
                assert_eq!(
                    plan.outcome(),
                    if fallback_expected {
                        "fallback"
                    } else {
                        "primary"
                    }
                );
                assert_eq!(
                    plan.projection(),
                    if fallback_expected {
                        "structured_text"
                    } else {
                        primary_projection
                    }
                );
                assert!(!plan.emitted_overflowed());
                assert!(plan.emitted_width() <= width);
                let metadata: serde_json::Value =
                    serde_json::from_slice(result.metadata_json()).unwrap();
                assert_eq!(metadata["output_plan"]["family"], "requirement");
                assert_eq!(metadata["output_plan"]["projection"], plan.projection());
                let text = std::str::from_utf8(result.data()).unwrap();
                assert!(!text.contains('\u{1b}'));
                let compact: String = text.split_whitespace().collect();
                for expected in [
                    "name=\"req\"id=\"R-1\"",
                    "text=\"Loginrequired\"risk=\"High\"verifyMethod=\"Test\"",
                    "N1[element]name=\"sys\"type=\"service\"docRef=\"spec.md\"",
                    "E1[\"satisfies\"]",
                ] {
                    assert!(compact.contains(expected), "missing {expected:?}: {text}");
                }
                let endpoints = if primary_projection == "structured_text" {
                    "source=[](unresolved)name=\"missing\"target=[R1]name=\"req\""
                } else {
                    "source=[N1]name=\"sys\"target=[R1]name=\"req\""
                };
                assert!(compact.contains(endpoints), "missing {endpoints:?}: {text}");
            }
        }

        let error = render_ascii(
            source.as_bytes(),
            br#"{"ascii":{"graphPresentation":"adaptive","colorMode":"ansi16"}}"#,
        )
        .expect_err("Requirement adaptive records require Plain encoding");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("graph_presentation"), "{error:?}");
    }

    #[test]
    fn styled_ascii_results_identify_their_exact_encoding() {
        for (color_mode, expected_encoding, required_bytes) in [
            ("ansi16", "ansi16", "\u{1b}["),
            ("ansi256", "ansi256", "\u{1b}[38;5;"),
            ("truecolor", "truecolor", "\u{1b}[38;2;"),
            ("html", "html", "<span style=\""),
        ] {
            let options = format!(
                r##"{{"ascii":{{"color_mode":"{color_mode}","theme":{{"foreground":"#010101","background":"#ffffff","line":"#020202","accent":"#030303","border":"#040404"}}}}}}"##
            );
            let result = crate::render_ascii_result(
                b"flowchart LR\nA[Hello] -- yes --> B[World]",
                options.as_bytes(),
            )
            .unwrap_or_else(|error| panic!("{color_mode} render should succeed: {error:?}"));
            let Some(crate::BindingOutputPlan::Ascii(plan)) = result.metadata().output_plan()
            else {
                panic!("ASCII operation must expose an ASCII output plan");
            };

            assert_eq!(plan.encoding(), expected_encoding);
            assert!(
                String::from_utf8_lossy(result.data()).contains(required_bytes),
                "{color_mode} bytes do not match metadata: {:?}",
                result.data()
            );
        }
    }

    #[test]
    fn binding_rejects_styled_complete_fallback_requests_before_rendering() {
        let error = render_ascii(
            b"flowchart LR\nA[Alpha] --> B[Beta]",
            br#"{ "ascii": { "color_mode": "ansi16", "max_width": 5, "overflow": "fallback" } }"#,
        )
        .expect_err("styled fallback combinations are not admitted");

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(
            error.message().contains("ascii_viewport.overflow"),
            "{error:?}"
        );
    }

    #[test]
    fn binding_adaptive_admission_uses_canonical_encoding_and_family_boundaries() {
        let error = render_ascii(
            b"flowchart LR\nA --> B",
            br#"{"ascii":{"graphPresentation":"adaptive","colorMode":"ansi16"}}"#,
        )
        .expect_err("adaptive graph requests require Plain encoding");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);

        let error = render_ascii(
            b"sequenceDiagram\nA->>B: Hello",
            br#"{"ascii":{"graph_presentation":"adaptive"}}"#,
        )
        .expect_err("a family outside the shared graph planner cannot opt into adaptive mode");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("graph_presentation"));

        let result = crate::render_ascii_result(
            b"flowchart LR\nA --> B",
            br#"{"ascii":{"colorMode":"ansi16"}}"#,
        )
        .expect("default strict graph presentation retains styled primary output");
        let Some(crate::BindingOutputPlan::Ascii(plan)) = result.metadata().output_plan() else {
            panic!("expected an ASCII output plan");
        };
        assert_eq!(plan.requested_graph_presentation(), "strict");
        assert_eq!(plan.encoding(), "ansi16");
    }

    #[test]
    fn ascii_viewport_aliases_and_invalid_width_are_strict() {
        for options in [
            br#"{ "ascii": { "max_width": 10, "overflow": "error" } }"#.as_slice(),
            br#"{ "ascii": { "maxWidth": 10, "overflow": "error" } }"#.as_slice(),
        ] {
            let error = render_ascii(b"flowchart LR\nA[Alpha] --> B[Beta]", options)
                .expect_err("error overflow policy should reject a wide graph");
            assert_eq!(error.status(), BindingStatus::RenderError);
            assert!(error.message().contains("width"), "{error:?}");
            let diagnostic = error
                .diagnostic_details()
                .expect("width overflow should expose numeric diagnostic details");
            assert_eq!(diagnostic.requested_max_width, Some(10));
            assert!(diagnostic.actual_width.is_some());
            assert_eq!(diagnostic.width_profile.as_deref(), Some("unicode"));
        }

        let error = render_ascii(b"flowchart TD\nA", br#"{ "ascii": { "maxWidth": 0 } }"#)
            .expect_err("zero max width must be rejected before rendering");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("max_width"), "{error:?}");

        let too_wide = crate::RUNTIME_CATALOG_MAX_SAFE_INTEGER + 1;
        let options = format!(r#"{{"ascii":{{"max_width":{too_wide}}}}}"#);
        let error = render_ascii(b"flowchart TD\nA", options.as_bytes())
            .expect_err("non-portable JSON widths must be rejected before rendering");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
    }

    #[test]
    fn render_ascii_parse_error_omits_authored_source() {
        let source = b"not-a-diagram\x1b]8;;https://example.invalid\x07link";

        let error = render_ascii(source, b"").expect_err("source should not detect as Mermaid");

        assert_eq!(error.status(), BindingStatus::ParseError);
        assert_eq!(error.message(), "No Mermaid diagram type detected");
        assert!(!error.message().contains("not-a-diagram"));
        assert!(!error.message().as_bytes().contains(&0x1b));
        assert!(!error.message().as_bytes().contains(&0x07));
        let diagnostic = error
            .diagnostic_details()
            .expect("detect failure should preserve diagnostic details");
        assert_eq!(diagnostic.code, "merman.parse.no_diagram_detected");
        assert_eq!(diagnostic.span, None);
    }

    #[test]
    fn ascii_parse_error_preserves_code_span_and_safe_context_in_binding_payload() {
        let span = merman::model::SourceSpan::new(3, 8);
        let diagnostic = merman::ParseDiagnostic::new("bad input")
            .with_span(span, merman::ParseDiagnosticSpanKind::InsertionPoint)
            .with_code("merman.test");
        let error = classify_render_error(
            merman::RenderError::from(merman::Error::diagram_parse_diagnostic(
                "flow\u{1b}",
                diagnostic,
            )),
            merman::model::resources::ResourceProfile::Interactive,
        );

        let details = error
            .diagnostic_details()
            .expect("parse failure should preserve diagnostic details");
        assert_eq!(details.code, "merman.test");
        assert_eq!(
            details.span,
            Some(crate::common::BindingDiagnosticSpan {
                start: 3,
                end: 8,
                kind: "insertion-point",
            })
        );
        assert_eq!(details.diagram_type.as_deref(), Some("flow\\u{1B}"));

        let payload: serde_json::Value =
            serde_json::from_slice(&crate::binding_error_payload_json_bytes(&error))
                .expect("binding error payload should be JSON");
        assert_eq!(payload["details"]["diagnostic"]["code"], "merman.test");
        assert_eq!(payload["details"]["diagnostic"]["span"]["start"], 3);
        assert_eq!(
            payload["details"]["diagnostic"]["span"]["kind"],
            "insertion-point"
        );
        assert_eq!(
            payload["details"]["diagnostic"]["diagram_type"],
            "flow\\u{1B}"
        );
    }

    #[test]
    fn nested_runtime_policy_errors_preserve_missing_capability_binding_classification() {
        let error = classify_render_error(
            merman::RenderError::from(merman::Error::RuntimePolicy(
                merman::model::runtime::RuntimePolicyError::MissingCapability(
                    merman::model::runtime::RuntimeCapability::SystemRandom,
                ),
            )),
            merman::model::resources::ResourceProfile::Interactive,
        );

        assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
        assert_eq!(error.capability_id(), Some("system-random"));
        assert_eq!(
            error
                .diagnostic_details()
                .expect("runtime-policy errors expose diagnostic details")
                .code,
            "merman.runtime_policy"
        );
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
    fn render_ascii_applies_flowchart_node_label_wrap_width_from_json() {
        for options_json in [
            br#"{ "ascii": { "flowchart_node_label_wrap_width": 8 } }"#.as_slice(),
            br#"{ "ascii": { "flowchartNodeLabelWrapWidth": 8 } }"#.as_slice(),
        ] {
            let text = String::from_utf8(
                render_ascii(b"flowchart TD\nA[\"Alpha Beta Gamma Delta\"]", options_json).unwrap(),
            )
            .unwrap();

            assert!(text.contains("Alpha"), "{text}");
            assert!(text.contains("Gamma"), "{text}");
            assert!(
                !text.contains("Alpha Beta Gamma Delta"),
                "flowchart node label should wrap before layout:\n{text}"
            );
        }
    }

    #[test]
    fn ascii_width_profile_compiles_from_snake_and_camel_case_json() {
        for options_json in [
            br#"{ "ascii": { "width_profile": "cjk" } }"#.as_slice(),
            br#"{ "ascii": { "widthProfile": "cjk" } }"#.as_slice(),
        ] {
            let options = crate::common::parse_options(options_json).expect("valid ASCII options");
            let compiled = ascii_options_from_json(&options).expect("ASCII options compile");

            assert_eq!(
                compiled.terminal_width_profile,
                merman::ascii::TerminalWidthProfile::Cjk
            );
        }
    }

    #[test]
    fn ascii_width_profile_rejects_unknown_values() {
        let options = crate::common::parse_options(
            br#"{ "ascii": { "width_profile": "terminal-default" } }"#,
        )
        .expect("JSON shape parses");

        let error = ascii_options_from_json(&options).expect_err("unknown profile is rejected");

        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(error.message().contains("ascii.width_profile"), "{error:?}");
    }

    #[test]
    fn ascii_layout_profile_accepts_snake_and_camel_case() {
        for options_json in [
            br#"{ "ascii": { "layout_profile": "compact" } }"#.as_slice(),
            br#"{ "ascii": { "layoutProfile": "compact" } }"#.as_slice(),
        ] {
            let options = crate::common::parse_options(options_json).expect("valid options");
            let compiled = ascii_options_from_json(&options).expect("ASCII options compile");
            assert_eq!(
                compiled.layout_profile,
                merman::ascii::AsciiLayoutProfile::Compact
            );
        }
    }

    #[test]
    fn ascii_graph_presentation_compiles_aliases_and_keeps_strict_default() {
        use merman::ascii::AsciiGraphPresentation;

        let defaults = crate::common::parse_options(b"").unwrap();
        assert_eq!(
            ascii_options_from_json(&defaults)
                .unwrap()
                .graph_presentation,
            AsciiGraphPresentation::Strict
        );
        for key in ["graph_presentation", "graphPresentation"] {
            for (value, expected) in [
                ("strict", AsciiGraphPresentation::Strict),
                ("adaptive", AsciiGraphPresentation::Adaptive),
                (" ADAPTIVE ", AsciiGraphPresentation::Adaptive),
            ] {
                let json = serde_json::json!({"ascii": {key: value}});
                let options =
                    crate::common::parse_options(&serde_json::to_vec(&json).unwrap()).unwrap();
                assert_eq!(
                    ascii_options_from_json(&options)
                        .unwrap()
                        .graph_presentation,
                    expected
                );
            }
        }
    }

    #[test]
    fn ascii_graph_presentation_rejects_unknown_values_and_invalid_types() {
        for key in ["graph_presentation", "graphPresentation"] {
            let json = serde_json::json!({"ascii": {key: "best_effort"}});
            let options =
                crate::common::parse_options(&serde_json::to_vec(&json).unwrap()).unwrap();
            let error = ascii_options_from_json(&options).unwrap_err();
            assert_eq!(error.status(), BindingStatus::InvalidArgument);
            assert!(error.message().contains("ascii.graph_presentation"));

            let json = serde_json::json!({"ascii": {key: true}});
            let error = crate::common::parse_options(&serde_json::to_vec(&json).unwrap())
                .expect_err("presentation must be a string");
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        }
    }

    #[test]
    fn adaptive_graph_presentation_preserves_fatal_resource_errors() {
        let source = b"flowchart LR\nA[Alpha] --> B[Beta]";
        let options = br#"{
            "ascii": {"graphPresentation": "adaptive", "overflow": "fallback", "maxWidth": 10},
            "resources": {"limits": {"max_ascii_grid_cells": 1}}
        }"#;
        let error = crate::render_ascii_result(source, options)
            .expect_err("resource exhaustion must not publish a recovery result");
        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        assert_eq!(
            error.resource_details().unwrap().limit_id,
            "max_ascii_grid_cells"
        );
        assert_eq!(crate::render_ascii(source, options).unwrap_err(), error);
        let payload: serde_json::Value =
            serde_json::from_slice(&crate::binding_error_payload_json_bytes(&error)).unwrap();
        assert_eq!(
            payload["details"]["resource"]["limit_id"],
            "max_ascii_grid_cells"
        );
    }

    #[test]
    fn auto_layout_aliases_select_and_round_trip_the_effective_profile() {
        let source = b"flowchart LR\nA --> B --> C --> D";
        let compact =
            crate::render_ascii_result(source, br#"{"ascii":{"layout_profile":"compact"}}"#)
                .expect("fixed compact render");
        let Some(crate::BindingOutputPlan::Ascii(compact_plan)) = compact.metadata().output_plan()
        else {
            panic!("ASCII output plan");
        };
        for key in ["layout_profile", "layoutProfile"] {
            let options = serde_json::json!({"ascii": {
                key: "auto", "max_width": compact_plan.primary_width(), "overflow": "error"
            }});
            let result = crate::render_ascii_result(source, &serde_json::to_vec(&options).unwrap())
                .expect("automatic selection should fit using compact");
            let parsed = crate::BindingOperationMetadata::from_json_bytes(result.metadata_json())
                .expect("schema 4 metadata must round-trip");
            let Some(crate::BindingOutputPlan::Ascii(plan)) = parsed.output_plan() else {
                panic!("ASCII output plan");
            };
            assert_eq!(plan.schema_version(), 4);
            assert_eq!(plan.requested_layout_profile(), "auto");
            assert_eq!(plan.layout_profile(), Some("compact"));
            assert!(plan.compact_attempted());
            assert_eq!(plan.primary_overflowed(), Some(false));
            assert!(!plan.emitted_overflowed());
            assert_eq!(plan.primary_width(), compact_plan.primary_width());
            assert_eq!(result.data(), compact.data());
        }
    }

    #[test]
    fn auto_layout_requires_width_and_supported_family() {
        for (source, options) in [
            (b"flowchart LR\nA --> B".as_slice(), br#"{"ascii":{"layout_profile":"auto"}}"#.as_slice()),
            (b"stateDiagram-v2\nA --> B".as_slice(), br#"{"ascii":{"layout_profile":"auto","max_width":80}}"#.as_slice()),
            (b"flowchart LR\nA --> B".as_slice(), br#"{"ascii":{"layout_profile":"auto","max_width":80,"overflow":"fallback","color_mode":"ansi16"}}"#.as_slice()),
        ] {
            let error = crate::render_ascii_result(source, options).expect_err("invalid automatic request");
            assert_eq!(error.status(), BindingStatus::InvalidArgument);
        }
    }

    #[test]
    fn ascii_layout_profile_rejects_unknown_values() {
        let options =
            crate::common::parse_options(br#"{ "ascii": { "layout_profile": "dense" } }"#)
                .expect("JSON shape parses");
        let error = ascii_options_from_json(&options).expect_err("unknown profile is rejected");
        assert_eq!(error.status(), BindingStatus::InvalidArgument);
        assert!(
            error.message().contains("ascii.layout_profile"),
            "{error:?}"
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
                    "flowchartNodeLabelWrapWidth": 5,
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
        assert_eq!(compiled.flowchart_node_label_wrap_width, 5);
        assert_eq!(compiled.sequence_participant_spacing, 6);
        assert_eq!(compiled.sequence_message_spacing, 7);
        assert_eq!(compiled.sequence_self_message_width, 8);
    }

    #[test]
    fn node_axis_padding_accepts_both_json_spellings_and_preserves_zero() {
        for json in [
            br#"{ "ascii": { "boxBorderPadding": 3, "node_padding_x": 2, "nodePaddingY": 0 } }"#
                .as_slice(),
            br#"{ "ascii": { "box_border_padding": 3, "nodePaddingX": 2, "node_padding_y": 0 } }"#
                .as_slice(),
        ] {
            let options = crate::common::parse_options(json).expect("parse node padding aliases");
            let compiled = ascii_options_from_json(&options).expect("zero padding compiles");
            assert_eq!(compiled.box_border_padding, 3);
            assert_eq!(compiled.node_padding_x, Some(2));
            assert_eq!(compiled.node_padding_y, Some(0));
        }
        let options = crate::common::parse_options(br#"{ "ascii": { "boxBorderPadding": 0 } }"#)
            .expect("parse legacy scalar padding");
        let compiled = ascii_options_from_json(&options).expect("legacy zero padding compiles");
        assert_eq!(compiled.box_border_padding, 0);
        assert_eq!(compiled.node_padding_x, None);
        assert_eq!(compiled.node_padding_y, None);
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
    fn render_ascii_relation_resource_limit_never_falls_back_to_summary() {
        let error = render_ascii(
            b"classDiagram\nclass Gateway\nclass Service\nclass Repo\nGateway --> Service : routes\nService --> Repo : stores",
            br#"{ "resources": { "limits": { "max_ascii_grid_cells": 1 } }, "ascii": { "charset": "ascii", "relationSummaryDiagnostics": true } }"#,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::ResourceLimitExceeded);
        let details = error
            .resource_details()
            .expect("structured resource details");
        assert_eq!(details.limit_id, "max_ascii_grid_cells");
        assert_eq!(details.phase, "ascii_layout");
        assert_eq!(details.max, 1);
        assert_eq!(details.profile, "interactive");
    }

    #[test]
    fn render_ascii_accepts_relation_summary_diagnostics_option() {
        let text = String::from_utf8(
            render_ascii(
                b"classDiagram\nclass A\nclass B\nclass C\nA --> B : ab\nB --> A : ba\nA --> C : ac\nC --> A : ca\nB --> C : bc\nC --> B : cb\nA --> A : self",
                br#"{ "resources": { "limits": { "max_ascii_grid_cells": 10000 } }, "ascii": { "charset": "ascii", "relationSummaryDiagnostics": true } }"#,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(text.contains("relations:"), "{text}");
        assert!(text.contains("reason: route_collision"), "{text}");
    }

    #[test]
    fn render_ascii_accepts_terminal_palette_theme_options() {
        let source = b"flowchart LR\nA -- yes --> B";
        let mut options = serde_json::json!({
            "ascii": {
                "color_mode": "truecolor",
                "theme": {
                    "foreground": "#010101",
                    "background": "#ffffff",
                    "line": "#020202",
                    "accent": "#030303",
                    "border": "#040404"
                }
            }
        });
        let bytes = render_ascii(source, &serde_json::to_vec(&options).unwrap()).unwrap();
        options["theme"] = serde_json::json!("studio");
        assert_eq!(
            render_ascii(source, &serde_json::to_vec(&options).unwrap()).unwrap(),
            bytes
        );
        let text = String::from_utf8(bytes).unwrap();

        assert!(text.contains("\u{1b}[38;2;1;1;1m"), "{text:?}");
        assert!(text.contains("\u{1b}[38;2;2;2;2m"), "{text:?}");
        assert!(text.contains("\u{1b}[38;2;3;3;3m"), "{text:?}");
        assert!(text.contains("\u{1b}[38;2;4;4;4m"), "{text:?}");
    }

    #[cfg(feature = "diagram-flowchart")]
    #[test]
    fn native_theme_only_changes_colored_ascii() {
        let source = b"flowchart LR\nA --> B";
        let render = |options| render_ascii(source, options).unwrap();
        assert_eq!(render(b""), render(br#"{"theme":"studio"}"#));
        let plain_color = render(br#"{"ascii":{"color_mode":"truecolor"}}"#);
        let native_color = render(br#"{"theme":{"preset":"studio","appearance":"dark"},"ascii":{"color_mode":"truecolor"}}"#);
        assert_ne!(plain_color, native_color);
        assert!(native_color.windows(5).any(|bytes| bytes == b"\x1b[38;"));
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
    fn render_ascii_rejects_zero_flowchart_node_label_wrap_width() {
        let err = render_ascii(
            b"flowchart TD\nA[Hello]",
            br#"{ "ascii": { "flowchartNodeLabelWrapWidth": 0 } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(
            err.message().contains("flowchart_node_label_wrap_width"),
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
    fn ascii_arithmetic_overflow_preserves_the_public_resource_cause() {
        let options = format!(
            r#"{{"resources":{{"profile":"unbounded-for-trusted-input"}},"ascii":{{"boxBorderPadding":{}}}}}"#,
            usize::MAX
        );
        let error = render_ascii(b"classDiagram\nclass A", options.as_bytes())
            .expect_err("box padding arithmetic must overflow before allocation");
        let resource = error
            .resource_details()
            .expect("ASCII overflow should expose structured details");
        assert_eq!(
            resource.cause,
            BindingResourceLimitCause::ArithmeticOverflow
        );
        assert_eq!(resource.limit_id, "max_ascii_grid_cells");
        assert_eq!(
            resource.actual,
            u64::try_from(usize::MAX).unwrap_or(u64::MAX)
        );
    }

    #[test]
    fn every_ascii_resource_limit_round_trips_exact_public_boundaries() {
        for case in ascii_resource_boundary_contract().binding_core_interactive {
            assert_binding_ascii_exact_boundary(&case.id, &case.phase, case.exact, &case.source);
        }
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
