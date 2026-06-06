use crate::common::{
    BindingError, BindingOptions, BindingStatus, binding_fixed_local_offset_minutes,
    binding_fixed_today, binding_site_config, css_declaration_value, finite_positive,
    internal_json_error, no_diagram_error, normalize_option, parse_options, source_text,
    validation_payload_json,
};
use merman::render::{
    DeterministicTextMeasurer, HeadlessRenderer, LayoutOptions, VendoredFontMetricsTextMeasurer,
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PipelineKind {
    Parity,
    Readable,
    ResvgSafe,
}

impl Default for PipelineKind {
    fn default() -> Self {
        Self::Parity
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SvgPipelineOptions {
    kind: PipelineKind,
    scoped_css: Option<String>,
    css_override_policy: merman::render::CssOverridePolicy,
    root_background_color: Option<String>,
    drop_native_duplicate_fallbacks: bool,
}

impl Default for SvgPipelineOptions {
    fn default() -> Self {
        Self {
            kind: PipelineKind::default(),
            scoped_css: None,
            css_override_policy: merman::render::CssOverridePolicy::Preserve,
            root_background_color: None,
            drop_native_duplicate_fallbacks: false,
        }
    }
}

impl SvgPipelineOptions {
    fn to_pipeline(self) -> merman::render::SvgPipeline {
        let mut pipeline = match self.kind {
            PipelineKind::Parity => merman::render::SvgPipeline::parity(),
            PipelineKind::Readable => merman::render::SvgPipeline::readable(),
            PipelineKind::ResvgSafe => merman::render::SvgPipeline::resvg_safe(),
        };

        if self.drop_native_duplicate_fallbacks {
            pipeline.push_postprocessor(merman::render::DropNativeDuplicateFallbacksPostprocessor);
        }

        if let Some(root_background_color) = self.root_background_color {
            pipeline.push_postprocessor(merman::render::RootBackgroundPostprocessor::new(
                root_background_color,
            ));
        }

        if let Some(scoped_css) = self.scoped_css.filter(|css| !css.trim().is_empty()) {
            pipeline.push_postprocessor(
                merman::render::ScopedCssPostprocessor::new(scoped_css)
                    .with_override_policy(self.css_override_policy),
            );
            if matches!(self.kind, PipelineKind::ResvgSafe) {
                pipeline.push_postprocessor(merman::render::SanitizeCssPostprocessor);
            }
        }

        pipeline
    }
}

pub fn render_svg(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = source_text(source)?;
    let options = parse_options(options_json)?;
    let (renderer, pipeline) = build_renderer(&options)?;

    let svg = renderer
        .render_svg_with_pipeline_sync(source, &pipeline.to_pipeline())
        .map_err(classify_render_error)?;

    match svg {
        Some(svg) => Ok(svg.into_bytes()),
        None => Err(no_diagram_error()),
    }
}

pub fn parse_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = source_text(source)?;
    let options = parse_options(options_json)?;
    let (renderer, _pipeline) = build_renderer(&options)?;

    let parsed = renderer
        .parse_diagram_sync(source)
        .map_err(classify_render_error)?
        .ok_or_else(no_diagram_error)?;

    serde_json::to_vec(&parsed.model).map_err(internal_json_error)
}

pub fn layout_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = source_text(source)?;
    let options = parse_options(options_json)?;
    let (renderer, _pipeline) = build_renderer(&options)?;

    let layouted = renderer
        .layout_diagram_sync(source)
        .map_err(classify_render_error)?
        .ok_or_else(no_diagram_error)?;

    serde_json::to_vec(&layouted).map_err(internal_json_error)
}

pub fn validate_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    validation_payload_json(parse_json(source, options_json).map(|_| ()))
}

#[derive(Clone)]
pub(crate) struct CachedRenderEngine {
    renderer: HeadlessRenderer,
    pipeline: merman::render::SvgPipeline,
}

impl CachedRenderEngine {
    pub(crate) fn new(options: &BindingOptions) -> Result<Self, BindingError> {
        let (renderer, pipeline) = build_renderer(options)?;
        Ok(Self {
            renderer,
            pipeline: pipeline.to_pipeline(),
        })
    }

    pub(crate) fn render_svg(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        let source = source_text(source)?;
        let svg = self
            .renderer
            .render_svg_with_pipeline_sync(source, &self.pipeline)
            .map_err(classify_render_error)?;

        match svg {
            Some(svg) => Ok(svg.into_bytes()),
            None => Err(no_diagram_error()),
        }
    }

    pub(crate) fn parse_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        let source = source_text(source)?;
        let parsed = self
            .renderer
            .parse_diagram_sync(source)
            .map_err(classify_render_error)?
            .ok_or_else(no_diagram_error)?;

        serde_json::to_vec(&parsed.model).map_err(internal_json_error)
    }

    pub(crate) fn layout_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        let source = source_text(source)?;
        let layouted = self
            .renderer
            .layout_diagram_sync(source)
            .map_err(classify_render_error)?
            .ok_or_else(no_diagram_error)?;

        serde_json::to_vec(&layouted).map_err(internal_json_error)
    }

    pub(crate) fn validate_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        validation_payload_json(self.parse_json(source).map(|_| ()))
    }
}

fn build_renderer(
    options: &BindingOptions,
) -> Result<(HeadlessRenderer, SvgPipelineOptions), BindingError> {
    let mut renderer = HeadlessRenderer::new()
        .with_fixed_today(binding_fixed_today(options)?)
        .with_fixed_local_offset_minutes(binding_fixed_local_offset_minutes(options)?);

    if options
        .parse
        .as_ref()
        .and_then(|parse| parse.suppress_errors)
        .unwrap_or(false)
    {
        renderer = renderer.with_lenient_parsing();
    } else {
        renderer = renderer.with_strict_parsing();
    }

    if let Some(site_config) = binding_site_config(options)? {
        renderer = renderer.with_site_config(site_config);
    }

    let mut layout = LayoutOptions::headless_svg_defaults();
    if let Some(layout_json) = options.layout.as_ref() {
        if let Some(width) = layout_json.viewport_width {
            layout.viewport_width = finite_positive(width, "layout.viewport_width")?;
        }
        if let Some(height) = layout_json.viewport_height {
            layout.viewport_height = finite_positive(height, "layout.viewport_height")?;
        }
        if let Some(kind) = layout_json.text_measurer.as_deref() {
            match normalize_option(kind).as_str() {
                "vendored" => {
                    layout.text_measurer = Arc::new(VendoredFontMetricsTextMeasurer::default());
                }
                "deterministic" => {
                    layout.text_measurer = Arc::new(DeterministicTextMeasurer::default());
                }
                other => {
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
                        format!("unsupported layout.text_measurer: {other}"),
                    ));
                }
            }
        }
    }
    renderer = renderer.with_layout_options(layout);

    if let Some(math_renderer) = options
        .layout
        .as_ref()
        .and_then(|layout| layout.math_renderer.as_deref())
    {
        match normalize_option(math_renderer).as_str() {
            "none" => {}
            "ratex" => {
                #[cfg(feature = "ratex-math")]
                {
                    renderer = renderer
                        .with_math_renderer(Arc::new(merman_render::math::RatexMathRenderer));
                }
                #[cfg(not(feature = "ratex-math"))]
                {
                    return Err(BindingError::new(
                        BindingStatus::UnsupportedFormat,
                        "layout.math_renderer=ratex requires the ratex-math feature",
                    ));
                }
            }
            other => {
                return Err(BindingError::new(
                    BindingStatus::InvalidArgument,
                    format!("unsupported layout.math_renderer: {other}"),
                ));
            }
        }
    }

    let mut pipeline = SvgPipelineOptions::default();
    if let Some(svg) = options.svg.as_ref() {
        if let Some(diagram_id) = svg.diagram_id.as_deref() {
            renderer = renderer.with_diagram_id(diagram_id);
        }
        if let Some(raw_pipeline) = svg.pipeline.as_deref() {
            pipeline.kind = match normalize_option(raw_pipeline).as_str() {
                "parity" => PipelineKind::Parity,
                "readable" => PipelineKind::Readable,
                "resvg-safe" | "resvg_safe" => PipelineKind::ResvgSafe,
                other => {
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
                        format!("unsupported svg.pipeline: {other}"),
                    ));
                }
            };
        }
        if let Some(raw_policy) = svg.css_override_policy.as_deref() {
            pipeline.css_override_policy = match normalize_option(raw_policy).as_str() {
                "preserve" => merman::render::CssOverridePolicy::Preserve,
                "strip-existing-important" | "strip_existing_important" => {
                    merman::render::CssOverridePolicy::StripExistingImportant
                }
                other => {
                    return Err(BindingError::new(
                        BindingStatus::InvalidArgument,
                        format!("unsupported svg.css_override_policy: {other}"),
                    ));
                }
            };
        }
        if let Some(scoped_css) = svg.scoped_css.as_deref() {
            pipeline.scoped_css = Some(scoped_css.to_string());
        }
        if let Some(root_background_color) = svg.root_background_color.as_deref() {
            pipeline.root_background_color = Some(css_declaration_value(
                root_background_color,
                "svg.root_background_color",
            )?);
        }
        pipeline.drop_native_duplicate_fallbacks =
            svg.drop_native_duplicate_fallbacks.unwrap_or(false);
    }

    Ok((renderer, pipeline))
}

fn classify_render_error(err: merman::render::HeadlessError) -> BindingError {
    match err {
        merman::render::HeadlessError::Parse(err) => {
            BindingError::new(BindingStatus::ParseError, err.to_string())
        }
        merman::render::HeadlessError::Render(err) => {
            BindingError::new(BindingStatus::RenderError, err.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn task_by_id<'a>(model: &'a Value, id: &str) -> &'a Value {
        model["tasks"]
            .as_array()
            .expect("Gantt tasks should be an array")
            .iter()
            .find(|task| task["id"].as_str() == Some(id))
            .unwrap_or_else(|| panic!("missing Gantt task {id} in {model}"))
    }

    #[test]
    fn render_svg_returns_svg_for_flowchart() {
        let svg =
            String::from_utf8(render_svg(b"flowchart TD\nA[Hello] --> B[World]", b"").unwrap())
                .unwrap();

        assert!(svg.contains("<svg"));
        assert!(svg.contains("Hello"));
        assert!(svg.contains("World"));
    }

    #[test]
    fn render_svg_accepts_options_json() {
        let options = br#"{
            "layout": { "text_measurer": "deterministic", "viewport_width": 640, "viewport_height": 480 },
            "svg": { "diagram_id": "bindings core diagram", "pipeline": "readable" }
        }"#;
        let svg =
            String::from_utf8(render_svg(b"flowchart TD\nA[Hello]", options).unwrap()).unwrap();

        assert!(svg.contains("id=\"bindings-core-diagram\""));
        assert!(svg.contains("data-merman-foreignobject"));
    }

    #[test]
    fn render_svg_accepts_external_site_config() {
        let options = br##"{
            "site_config": {
                "theme": "base",
                "themeVariables": {
                    "mainBkg": "#111827",
                    "nodeTextColor": "#f8fafc",
                    "nodeBorder": "#38bdf8"
                },
                "themeCSS": ".node rect { filter: drop-shadow(1px 1px 1px #000); }"
            },
            "svg": { "diagram_id": "bindings theme config" }
        }"##;
        let svg = String::from_utf8(render_svg(b"flowchart TD\nA[Plain source]", options).unwrap())
            .unwrap();

        assert!(svg.contains("#111827"), "{svg}");
        assert!(svg.contains("#f8fafc"), "{svg}");
        assert!(svg.contains("#38bdf8"), "{svg}");
        assert!(
            svg.contains(
                "#bindings-theme-config .node rect { filter: drop-shadow(1px 1px 1px #000); }"
            ),
            "{svg}"
        );
        assert!(svg.contains(r#"data-merman-postprocess="scoped-css""#));
    }

    #[test]
    fn non_object_site_config_returns_invalid_argument() {
        let err =
            render_svg(b"flowchart TD\nA[Hello]", br#"{ "site_config": "dark" }"#).unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("site_config"));
    }

    #[test]
    fn svg_options_can_inject_host_scoped_css() {
        let options = br##"{
            "svg": {
                "diagram_id": "bindings host css",
                "scoped_css": ".node rect { fill: #abcdef; }"
            }
        }"##;
        let svg = String::from_utf8(render_svg(b"flowchart TD\nA[Plain source]", options).unwrap())
            .unwrap();

        assert!(svg.contains(r#"data-merman-postprocess="scoped-css""#));
        assert!(
            svg.contains("#bindings-host-css .node rect { fill: #abcdef; }"),
            "{svg}"
        );
    }

    #[test]
    fn svg_options_scoped_css_can_strip_existing_important() {
        let options = parse_options(
            br##"{
                "svg": {
                    "pipeline": "parity",
                    "scoped_css": ".node { fill: #00ff00; }",
                    "css_override_policy": "strip-existing-important"
                }
            }"##,
        )
        .unwrap();
        let (_renderer, pipeline) = build_renderer(&options).unwrap();
        let out = pipeline
            .to_pipeline()
            .process_to_string(
                r#"<svg id="host"><style>.node{fill:red !important;}</style><g/></svg>"#,
            )
            .unwrap();

        assert!(!out.contains("!important"), "{out}");
        assert!(out.contains("#host .node { fill: #00ff00; }"));
    }

    #[test]
    fn resvg_safe_scoped_css_is_sanitized_after_injection() {
        let options = parse_options(
            br##"{
                "svg": {
                    "pipeline": "resvg-safe",
                    "scoped_css": "@keyframes dash { to { stroke-dashoffset: 10; } } .edge { animation: dash 1s; transform: rotate(45deg); }"
                }
            }"##,
        )
        .unwrap();
        let (_renderer, pipeline) = build_renderer(&options).unwrap();
        let out = pipeline
            .to_pipeline()
            .process_to_string(r#"<svg id="host"><path class="edge"/></svg>"#)
            .unwrap();

        assert!(!out.contains("@keyframes"), "{out}");
        assert!(!out.contains("animation"), "{out}");
        assert!(!out.contains("45deg"), "{out}");
        assert!(out.contains("#host .edge"));
    }

    #[test]
    fn svg_options_can_set_root_background_color() {
        let options = parse_options(
            br##"{
                "svg": {
                    "root_background_color": "#111827"
                }
            }"##,
        )
        .unwrap();
        let (_renderer, pipeline) = build_renderer(&options).unwrap();
        let out = pipeline
            .to_pipeline()
            .process_to_string(
                r#"<svg id="host" style="max-width: 400px; background-color: white;"><g/></svg>"#,
            )
            .unwrap();

        assert_eq!(
            out,
            r#"<svg id="host" style="max-width: 400px; background-color: #111827;"><g/></svg>"#
        );
    }

    #[test]
    fn invalid_root_background_color_returns_invalid_argument() {
        let err = render_svg(
            b"flowchart TD\nA[Hello]",
            br##"{ "svg": { "root_background_color": "white; color: red" } }"##,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("svg.root_background_color"));
    }

    #[test]
    fn invalid_css_override_policy_returns_invalid_argument() {
        let err = render_svg(
            b"flowchart TD\nA[Hello]",
            br#"{ "svg": { "css_override_policy": "remove-everything" } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("svg.css_override_policy"));
    }

    #[test]
    fn svg_options_can_drop_native_duplicate_fallbacks() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
<text class="task">Make tea</text>
<g transform="translate(0,0)">
  <foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Make tea</p></div></foreignObject>
</g>
<g transform="translate(0,40)">
  <foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Only fallback</p></div></foreignObject>
</g>
</svg>"##;

        let default_options = parse_options(br#"{"svg":{"pipeline":"resvg-safe"}}"#).unwrap();
        let (_renderer, default_pipeline) = build_renderer(&default_options).unwrap();
        let default_out = default_pipeline
            .to_pipeline()
            .process_to_string(svg)
            .unwrap();
        assert_eq!(
            default_out
                .matches(r#"data-merman-foreignobject="fallback""#)
                .count(),
            2,
            "{default_out}"
        );

        let cleanup_options = parse_options(
            br#"{"svg":{"pipeline":"resvg-safe","drop_native_duplicate_fallbacks":true}}"#,
        )
        .unwrap();
        let (_renderer, cleanup_pipeline) = build_renderer(&cleanup_options).unwrap();
        let cleanup_out = cleanup_pipeline
            .to_pipeline()
            .process_to_string(svg)
            .unwrap();

        assert_eq!(
            cleanup_out
                .matches(r#"data-merman-foreignobject="fallback""#)
                .count(),
            1,
            "{cleanup_out}"
        );
        assert!(cleanup_out.contains("Only fallback"));
        assert!(cleanup_out.contains(r#"<text class="task">Make tea</text>"#));
        assert!(!cleanup_out.contains("<foreignObject"));
    }

    #[test]
    fn parse_json_returns_semantic_model() {
        let json: Value = serde_json::from_slice(
            &parse_json(b"flowchart TD\nA[Hello] --> B[World]", b"").unwrap(),
        )
        .unwrap();

        assert_eq!(
            json.get("type").and_then(Value::as_str),
            Some("flowchart-v2")
        );
        assert!(json.get("nodes").and_then(Value::as_array).is_some());
        assert!(json.get("edges").and_then(Value::as_array).is_some());
    }

    #[test]
    fn parse_json_accepts_fixed_time_options() {
        let source = br#"gantt
dateFormat MM-DD
section Demo
Missing year: id1,03-01,1d
Missing ref: id2,after missing,1d
"#;
        let options = br#"{
            "fixed_today": "2026-02-15",
            "fixed_local_offset_minutes": 0
        }"#;
        let json: Value = serde_json::from_slice(&parse_json(source, options).unwrap()).unwrap();

        assert_eq!(
            task_by_id(&json, "id1")["startTime"].as_i64(),
            Some(1_772_323_200_000)
        );
        assert_eq!(
            task_by_id(&json, "id2")["startTime"].as_i64(),
            Some(1_771_113_600_000)
        );
    }

    #[test]
    fn render_svg_accepts_fixed_time_options() {
        let source = br#"gantt
dateFormat YYYY-MM-DD
section Demo
Anchor: id1,2026-01-01,1d
Missing ref: id2,after missing,1d
"#;
        let first = render_svg(
            source,
            br#"{
                "fixed_today": "2026-02-15",
                "fixed_local_offset_minutes": 0,
                "svg": { "diagram_id": "bindings-fixed-gantt" }
            }"#,
        )
        .unwrap();
        let second = render_svg(
            source,
            br#"{
                "fixed_today": "2026-03-15",
                "fixed_local_offset_minutes": 0,
                "svg": { "diagram_id": "bindings-fixed-gantt" }
            }"#,
        )
        .unwrap();

        assert_ne!(
            first, second,
            "Gantt SVG output should reflect binding fixed-time options"
        );
    }

    #[test]
    fn invalid_fixed_time_options_return_invalid_argument() {
        for (options, expected) in [
            (
                br#"{ "fixed_today": "2026/02/15" }"#.as_slice(),
                "fixed_today",
            ),
            (
                br#"{ "fixed_local_offset_minutes": 1440 }"#.as_slice(),
                "fixed_local_offset_minutes",
            ),
        ] {
            let err = parse_json(b"flowchart TD\nA[Hello]", options).unwrap_err();

            assert_eq!(err.status(), BindingStatus::InvalidArgument);
            assert!(err.message().contains(expected), "{err:?}");
        }
    }

    #[test]
    fn layout_json_returns_layouted_diagram() {
        let json: Value = serde_json::from_slice(
            &layout_json(b"flowchart TD\nA[Hello] --> B[World]", b"").unwrap(),
        )
        .unwrap();

        assert!(json.get("meta").is_some());
        assert!(json.get("layout").is_some());
    }

    #[test]
    fn validate_json_reports_success_and_errors_without_throwing() {
        let valid: Value =
            serde_json::from_slice(&validate_json(b"flowchart TD\nA[Hello]", b"").unwrap())
                .unwrap();
        assert_eq!(valid["valid"], true);
        assert_eq!(valid["code_name"], BindingStatus::Ok.code_name());
        assert_eq!(valid.get("error"), Some(&Value::Null));

        let invalid: Value = serde_json::from_slice(&validate_json(b"", b"").unwrap()).unwrap();
        assert_eq!(invalid["valid"], false);
        assert_eq!(invalid["code_name"], BindingStatus::NoDiagram.code_name());
        assert!(
            invalid["error"]
                .as_str()
                .unwrap()
                .contains("no Mermaid diagram")
        );
    }

    #[test]
    fn invalid_source_utf8_returns_utf8_error() {
        let err = render_svg(&[0xff], b"").unwrap_err();

        assert_eq!(err.status(), BindingStatus::Utf8Error);
        assert!(err.message().contains("invalid source UTF-8"));
    }

    #[test]
    fn invalid_options_json_returns_options_json_error() {
        let err = render_svg(b"flowchart TD\nA", b"{").unwrap_err();

        assert_eq!(err.status(), BindingStatus::OptionsJsonError);
        assert!(err.message().contains("invalid options_json"));
    }

    #[test]
    fn empty_source_returns_no_diagram() {
        let err = render_svg(b"", b"").unwrap_err();

        assert_eq!(err.status(), BindingStatus::NoDiagram);
    }

    #[test]
    fn invalid_option_value_returns_invalid_argument() {
        let err = render_svg(
            b"flowchart TD\nA",
            br#"{ "layout": { "viewport_width": -1 } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("layout.viewport_width"));
    }

    #[test]
    fn unsupported_ratex_without_feature_returns_unsupported_format() {
        let result = render_svg(
            b"flowchart TD\nA[Hello]",
            br#"{ "layout": { "math_renderer": "ratex" } }"#,
        );

        if cfg!(feature = "ratex-math") {
            assert!(result.is_ok());
        } else {
            let err = result.unwrap_err();
            assert_eq!(err.status(), BindingStatus::UnsupportedFormat);
        }
    }
}
