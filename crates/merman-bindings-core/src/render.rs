mod request;

#[cfg(test)]
use crate::common::parse_options;
use crate::common::{BindingError, BindingOptions, source_text};
use request::RenderRequestPlan;

pub fn render_svg(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("svg", source, options_json)
}

pub fn layout_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("layout-json", source, options_json)
}

pub fn edge_geometry_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("edge-geometry-json", source, options_json)
}

#[derive(Clone)]
pub(crate) struct CachedRenderEngine {
    plan: RenderRequestPlan,
}

pub(crate) struct RenderOperationConfig {
    plan: request::RenderOperationConfig,
}

impl CachedRenderEngine {
    pub(crate) fn render_svg_output(
        &self,
        source: &[u8],
        control: merman::OperationControl,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let source = source_text(source)?;
        self.plan.render_svg_output(source, control)
    }

    pub(crate) fn layout_json(
        &self,
        source: &[u8],
        control: merman::OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        let source = source_text(source)?;
        self.plan.layout_json(source, control)
    }

    pub(crate) fn edge_geometry_json(
        &self,
        source: &[u8],
        control: merman::OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        let source = source_text(source)?;
        self.plan.edge_geometry_json(source, control)
    }

    pub(crate) fn svg_plan_json(
        &self,
        source: &[u8],
        control: merman::OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        let source = source_text(source)?;
        self.plan.svg_plan_json(source, control)
    }

    #[cfg(feature = "png")]
    pub(crate) fn render_png_output(
        &self,
        source: &[u8],
        control: merman::OperationControl,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let source = source_text(source)?;
        self.plan.render_png_output(source, control)
    }

    #[cfg(feature = "jpeg")]
    pub(crate) fn render_jpeg_output(
        &self,
        source: &[u8],
        control: merman::OperationControl,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let source = source_text(source)?;
        self.plan.render_jpeg_output(source, control)
    }

    #[cfg(feature = "pdf")]
    pub(crate) fn render_pdf_output(
        &self,
        source: &[u8],
        control: merman::OperationControl,
    ) -> Result<crate::operation::BindingOperationOutput, BindingError> {
        let source = source_text(source)?;
        self.plan.render_pdf_output(source, control)
    }
}

impl RenderOperationConfig {
    pub(crate) fn compile(
        options: &BindingOptions,
        runtime_policy: merman::runtime::RuntimePolicy,
        capability_policy: merman::svg::RenderCapabilityPolicy,
        theme: Option<merman::svg::DiagramTheme>,
        theme_resources: merman::svg::ThemeResourcePolicy,
    ) -> Result<Self, BindingError> {
        Ok(Self {
            plan: request::RenderOperationConfig::compile(
                options,
                runtime_policy,
                capability_policy,
                theme,
                theme_resources,
            )?,
        })
    }

    pub(crate) fn materialize(self, services: &crate::BindingEngineServices) -> CachedRenderEngine {
        CachedRenderEngine {
            plan: self.plan.materialize(services),
        }
    }
}

#[cfg(feature = "png")]
pub fn render_png(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("png", source, options_json)
}

#[cfg(feature = "jpeg")]
pub fn render_jpeg(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("jpeg", source, options_json)
}

#[cfg(feature = "pdf")]
pub fn render_pdf(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    execute_once_data("pdf", source, options_json)
}

fn execute_once_data(
    operation_id: &str,
    source: &[u8],
    options_json: &[u8],
) -> Result<Vec<u8>, BindingError> {
    crate::execute_once_data(operation_id, source, None, options_json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BindingStatus;
    use serde_json::Value;

    fn render_session() -> merman_render::environment::RenderSession {
        merman_render::environment::RenderEnvironment::deterministic()
            .begin_session()
            .expect("render session")
    }

    fn root_style_property_is(svg: &str, property: &str, expected: &str) -> bool {
        let Ok(document) = roxmltree::Document::parse(svg) else {
            return false;
        };
        document
            .root_element()
            .attribute("style")
            .is_some_and(|style| {
                style.split(';').map(str::trim).any(|declaration| {
                    declaration.split_once(':').is_some_and(|(name, value)| {
                        name.trim() == property && value.trim() == expected
                    })
                })
            })
    }

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
    fn render_svg_flowchart_elk_follows_the_artifact_owner_feature() {
        let result = render_svg(b"flowchart-elk TD\nA[Hello] --> B[World]", b"");

        if cfg!(feature = "layout-elk") {
            let svg = String::from_utf8(result.unwrap()).expect("SVG is UTF-8");
            assert!(svg.contains("<svg"));
            assert!(svg.contains("Hello"));
            assert!(svg.contains("World"));
            assert!(!svg.contains("NaN"));
        } else {
            let err =
                result.expect_err("ELK must be denied when the artifact owner did not select it");
            assert_eq!(err.status(), BindingStatus::UnsupportedOperation);
            assert!(err.message().contains("layout-elk"), "{err:?}");
        }
    }

    #[cfg(feature = "svg")]
    #[test]
    fn render_svg_flowchart_elk_does_not_follow_ambient_dependency_features() {
        if cfg!(feature = "layout-elk") {
            return;
        }

        let result = render_svg(
            b"---\nconfig:\n  layout: elk\n---\nflowchart TD\nA[Hello] --> B[World]",
            b"",
        );
        let err = result.expect_err("ambient ELK must not widen the artifact contract");
        assert_eq!(err.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(err.capability_id(), Some("layout-elk"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn render_svg_architecture_does_not_follow_ambient_cytoscape_features() {
        if cfg!(feature = "layout-cytoscape") {
            return;
        }

        let result = render_svg(b"architecture-beta\n  service api(server)[API]", b"");
        let err = result.expect_err("ambient Cytoscape must not widen the artifact contract");
        assert_eq!(err.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(err.capability_id(), Some("layout-cytoscape"));
    }

    #[cfg(feature = "svg")]
    #[test]
    fn binding_math_renderer_follows_the_artifact_owner_feature() {
        let source = b"flowchart TD\nA[\"$$x^2$$\"] --> B[Done]";
        let default = render_svg(source, b"");

        if cfg!(feature = "math") {
            assert!(
                default.is_ok(),
                "the owner-selected math capability must be usable by default"
            );
        } else {
            assert_missing_math(default.unwrap_err());
        }

        let disabled = render_svg(source, br#"{"environment":{"math_renderer":"none"}}"#);
        assert_missing_math(disabled.unwrap_err());
    }

    #[test]
    fn render_svg_accepts_options_json() {
        let options = br#"{
            "layout": {
                "container_width": 640,
                "container_height": 480,
                "screen_available_width": 1280
            },
            "environment": { "text_measurement": "deterministic" },
            "svg": { "diagram_id": "bindings core diagram", "pipeline": "readable" }
        }"#;
        let svg =
            String::from_utf8(render_svg(b"flowchart TD\nA[Hello]", options).unwrap()).unwrap();

        assert!(svg.contains("id=\"bindings-core-diagram\""));
        assert!(svg.contains("data-merman-foreignobject"));
    }

    #[cfg(feature = "svg")]
    fn assert_missing_math(error: BindingError) {
        assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
        assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
        assert_eq!(error.capability_id(), Some("math"));
    }

    #[test]
    fn render_svg_resvg_safe_pipeline_selects_export_contract() {
        let source = b"flowchart TD
A[Start] --> B{Is it working?}
B -->|Yes| C[Ship it]
B -->|No| D[Debug]";
        let parity_svg = String::from_utf8(render_svg(source, b"").unwrap()).unwrap();
        let export_svg =
            String::from_utf8(render_svg(source, br#"{"svg":{"pipeline":"resvg-safe"}}"#).unwrap())
                .unwrap();

        assert!(
            parity_svg.contains("<foreignObject"),
            "default binding SVG should preserve Mermaid HTML label DOM: {parity_svg}"
        );
        assert!(
            !export_svg.contains("<foreignObject"),
            "resvg-safe binding SVG should not rely on foreignObject: {export_svg}"
        );
        assert!(
            export_svg.contains(r#"data-merman-foreignobject="fallback""#),
            "resvg-safe binding SVG should keep generated text fallbacks: {export_svg}"
        );
        for label in ["Start", "Is it working?", "Yes", "Ship it", "No", "Debug"] {
            assert!(
                export_svg.contains(label),
                "resvg-safe binding SVG should keep visible label {label:?}: {export_svg}"
            );
        }
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
                }
            },
            "svg": { "diagram_id": "bindings theme config" }
        }"##;
        let svg = String::from_utf8(render_svg(b"flowchart TD\nA[Plain source]", options).unwrap())
            .unwrap();

        assert!(svg.contains("#111827"), "{svg}");
        assert!(svg.contains("#f8fafc"), "{svg}");
        assert!(svg.contains("#38bdf8"), "{svg}");
    }

    #[test]
    fn general_bindings_reject_raw_theme_css() {
        let error = render_svg(
            b"flowchart TD\nA[Plain source]",
            br#"{"site_config":{"themeCSS":".node rect { fill: red; }"}}"#,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("site_config.themeCSS"));
        assert!(error.message().contains("typed `theme` schema"));
    }

    #[test]
    fn general_bindings_reject_unsafe_theme_variable_css() {
        let error = render_svg(
            b"flowchart TD\nA[Plain source]",
            br##"{
                "site_config": {
                    "themeVariables": {
                        "mainBkg": "#fff;}</style><script>alert(1)</script>"
                    }
                }
            }"##,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("themeVariables"), "{error:?}");
    }

    #[test]
    fn one_shot_binding_rejects_site_config_secure_override() {
        let error = render_svg(
            br##"%%{init: {"themeCSS": ".node rect { fill: red; }"}}%%
flowchart TD
A[Plain source]"##,
            br#"{"site_config":{"secure":[]}}"#,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("site_config.secure"), "{error:?}");
        assert!(error.message().contains("trusted Rust or native CLI host"));
    }

    #[test]
    fn reusable_binding_constructor_rejects_site_config_secure_override() {
        let error = match crate::BindingEngine::new(br#"{"site_config":{"secure":[]}}"#) {
            Ok(_) => panic!("general bindings must not let callers replace secure keys"),
            Err(error) => error,
        };

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("site_config.secure"), "{error:?}");
        assert!(error.message().contains("trusted Rust or native CLI host"));
    }

    #[test]
    fn reusable_request_overlay_rejects_site_config_secure_override() {
        let engine = crate::BindingEngine::new(b"").unwrap();
        let error = engine
            .execute(
                crate::BindingOperationRequest::new(
                    "svg",
                    b"---\nconfig:\n  themeCSS: '.node rect { fill: red; }'\n---\nflowchart TD\nA",
                )
                .with_options_json(br#"{"site_config":{"secure":[]}}"#),
            )
            .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("site_config.secure"), "{error:?}");
        assert!(error.message().contains("trusted Rust or native CLI host"));
    }

    #[test]
    fn render_svg_accepts_typed_theme_and_independent_svg_output() {
        let options = br##"{
            "theme": {
                "spec": {
                    "typography": {
                        "default": {
                            "font_stack": ["system-ui", "sans-serif"],
                            "font_size_px": 17.0
                        }
                    },
                    "styles": [
                        {
                            "kind": "rule",
                            "target": "node",
                            "family": "flowchart",
                            "style": {
                                "fill": "#111827",
                                "stroke": { "paint": "#475569", "width": 2.0 },
                                "radius": 6.0
                            }
                        },
                        {
                            "kind": "rule",
                            "target": "node-label",
                            "family": "flowchart",
                            "style": {
                                "typography": { "font_size_px": 17.0 }
                            }
                        },
                        {
                            "kind": "rule",
                            "target": "edge",
                            "family": "flowchart",
                            "style": { "stroke": { "paint": "#94a3b8" } }
                        }
                    ],
                    "canvas": { "base": "#0f172a" }
                }
            },
            "svg": {
                "diagram_id": "bindings typed theme",
                "pipeline": "resvg-safe",
                "root_background_color": "#0f172a",
                "drop_native_duplicate_fallbacks": true
            }
        }"##;
        let svg =
            String::from_utf8(render_svg(b"flowchart TD\nA[Typed] --> B[Theme]", options).unwrap())
                .unwrap();

        assert!(svg.contains(r#"id="bindings-typed-theme""#), "{svg}");
        assert!(svg.contains("#111827"), "{svg}");
        assert!(svg.contains("#475569"), "{svg}");
        assert!(svg.contains("#94a3b8"), "{svg}");
        assert!(
            root_style_property_is(&svg, "background-color", "#0f172a"),
            "{svg}"
        );
        assert!(!svg.contains("<foreignObject"), "{svg}");
    }

    #[test]
    fn explicit_site_config_overrides_compiled_theme_variables() {
        let options = br##"{
            "theme": {
                "spec": {
                    "styles": [{
                        "kind": "rule",
                        "target": "node",
                        "family": "flowchart",
                        "style": {
                            "fill": "#111111",
                            "stroke": { "paint": "#222222" }
                        }
                    }]
                }
            },
            "site_config": {
                "themeVariables": {
                    "nodeBorder": "#abcdef"
                }
            },
            "svg": { "diagram_id": "bindings host override" }
        }"##;
        let svg =
            String::from_utf8(render_svg(b"flowchart TD\nA[Host]", options).unwrap()).unwrap();

        assert!(svg.contains("#abcdef"), "{svg}");
    }

    #[test]
    fn theme_preset_applies_common_editor_theme() {
        let svg = String::from_utf8(
            render_svg(
                b"flowchart TD\nA[One Dark] --> B[Readable]",
                br##"{
                    "theme": { "preset": "one-dark" },
                    "svg": {
                        "diagram_id": "bindings one dark",
                        "root_background_color": "#282c34"
                    }
                }"##,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(svg.contains("#282c34"), "{svg}");
        assert!(svg.contains("#abb2bf"), "{svg}");
        assert!(svg.contains("#61afef"), "{svg}");
        assert!(
            root_style_property_is(&svg, "background-color", "#282c34"),
            "{svg}"
        );
    }

    #[test]
    fn json_theme_matches_the_equivalent_rust_theme() {
        let source = "flowchart TD\nA[One Dark] --> B[Typed]";
        let options = br##"{
            "theme": { "preset": "one-dark" },
            "site_config": {
                "layout": "dagre"
            },
            "svg": { "diagram_id": "theme equivalence" }
        }"##;
        let binding_svg = String::from_utf8(render_svg(source.as_bytes(), options).unwrap())
            .expect("binding SVG should be UTF-8");

        let theme = merman::svg::DiagramThemeCompiler::new()
            .compile_preset(merman::svg::ThemePreset::OneDark)
            .unwrap();
        let environment = merman::SvgEnvironment::deterministic();
        let renderer = merman::Renderer::new().with_engine(merman::Engine::new().with_site_config(
            merman::MermaidConfig::from_value(serde_json::json!({
                "layout": "dagre",
            })),
        ));
        let svg_request = merman::SvgRequest {
            environment,
            options: merman::svg::SvgRenderOptions {
                diagram_id: Some("theme equivalence".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };
        let rust_svg = renderer
            .render(
                merman::RenderRequest::svg(
                    source,
                    merman::OperationControl::new(),
                    svg_request.clone(),
                )
                .with_theme(theme.clone()),
            )
            .expect("Rust rendering should succeed");
        let merman::RenderOutput::Svg(Some(rust_svg)) = rust_svg else {
            panic!("typed SVG request should produce SVG output");
        };
        assert_eq!(binding_svg, rust_svg.svg());

        let binding_plan: Value = serde_json::from_slice(
            &crate::svg_plan_json(source.as_bytes(), options).expect("binding plan should succeed"),
        )
        .expect("binding plan should be JSON");
        let rust_plan = renderer
            .render(
                merman::RenderRequest::svg_plan(
                    source,
                    merman::OperationControl::new(),
                    svg_request,
                )
                .with_theme(theme),
            )
            .expect("Rust planning should succeed");
        let merman::RenderOutput::SvgPlan(Some(rust_plan)) = rust_plan else {
            panic!("typed SVG-plan request should produce a capability plan");
        };
        let mut required = rust_plan.required_capability_ids().collect::<Vec<_>>();
        required.sort_unstable();
        let mut missing = rust_plan.missing_capability_ids().collect::<Vec<_>>();
        missing.sort_unstable();
        assert_eq!(binding_plan["diagram_type"], rust_plan.diagram_type());
        assert_eq!(
            binding_plan["required_capability_ids"],
            serde_json::json!(required)
        );
        assert_eq!(
            binding_plan["missing_capability_ids"],
            serde_json::json!(missing)
        );
        assert_eq!(binding_plan["ready"], rust_plan.is_ready());
    }

    #[test]
    fn invalid_theme_preset_returns_invalid_argument() {
        let err = render_svg(
            b"flowchart TD\nA[Host]",
            br##"{ "theme": { "preset": "solarized-maybe" } }"##,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("theme.preset"));
    }

    #[test]
    fn theme_union_rejects_empty_mixed_and_null_payloads() {
        for (options, field) in [
            (r#"{ "theme": {} }"#, "exactly one"),
            (
                r#"{ "theme": { "preset": "editor-dark", "spec": {} } }"#,
                "both",
            ),
            (r#"{ "theme": { "preset": null } }"#, "must not be null"),
            (r#"{ "theme": { "spec": null } }"#, "must not be null"),
        ] {
            let err = render_svg(b"flowchart TD\nA[Host]", options.as_bytes()).unwrap_err();
            assert_eq!(err.status(), BindingStatus::OptionsJsonError);
            assert!(err.message().contains(field), "{err:?}");
        }
    }

    #[test]
    fn removed_presentation_inputs_point_to_the_typed_theme_owner() {
        for (options, removed) in [
            (
                br##"{ "host_theme": { "preset": "one-dark" } }"##.as_slice(),
                "host_theme",
            ),
            (
                br##"{ "presentation": { "profile": "merman-modern" } }"##.as_slice(),
                "presentation",
            ),
        ] {
            let error = render_svg(b"flowchart TD\nA[Host]", options).unwrap_err();
            assert_eq!(error.status(), BindingStatus::OptionsJsonError);
            assert!(error.message().contains(removed));
            assert!(error.message().contains("top-level `theme`"));
        }
    }

    #[test]
    fn one_shot_binding_rejects_css_override_policy() {
        let error = render_svg(
            b"flowchart TD\nA[Plain source]",
            br#"{"svg":{"css_override_policy":"preserve"}}"#,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(
            error.message().contains("svg.css_override_policy"),
            "{error:?}"
        );
        assert!(error.message().contains("trusted Rust or native CLI host"));
    }

    #[test]
    fn invalid_typed_theme_color_returns_invalid_argument() {
        let err = render_svg(
            b"flowchart TD\nA[Host]",
            br##"{
                "theme": {
                    "spec": {
                        "canvas": { "base": "white; color: red" }
                    }
                }
            }"##,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("theme.spec.paint"));
    }

    #[test]
    fn non_object_site_config_returns_invalid_argument() {
        let err =
            render_svg(b"flowchart TD\nA[Hello]", br#"{ "site_config": "dark" }"#).unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("site_config"));
    }

    #[test]
    fn one_shot_binding_rejects_snake_case_scoped_css() {
        let error = render_svg(
            b"flowchart TD\nA[Plain source]",
            br##"{
                "svg": {
                    "diagram_id": "bindings host css",
                    "scoped_css": ".node rect { fill: #abcdef; }"
                }
            }"##,
        )
        .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("svg.scoped_css"), "{error:?}");
        assert!(error.message().contains("trusted Rust or native CLI host"));
    }

    #[test]
    fn reusable_binding_constructor_rejects_camel_case_scoped_css() {
        let error = match crate::BindingEngine::new(
            br##"{
                "svg": {
                    "pipeline": "parity",
                    "scopedCss": ".node { fill: #00ff00; }"
                }
            }"##,
        ) {
            Ok(_) => panic!("general bindings must reject raw scoped CSS at construction"),
            Err(error) => error,
        };

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("svg.scopedCss"), "{error:?}");
        assert!(error.message().contains("trusted Rust or native CLI host"));
    }

    #[test]
    fn reusable_request_overlay_cannot_inject_scoped_css() {
        let engine = crate::BindingEngine::new(b"").unwrap();
        let error = engine
            .execute(
                crate::BindingOperationRequest::new("svg", b"flowchart TD\nA[Plain source]")
                    .with_options_json(
                        br##"{
                            "svg": {
                                "pipeline": "resvg-safe",
                                "scoped_css": "@keyframes dash { to { stroke-dashoffset: 10; } }"
                            }
                        }"##,
                    ),
            )
            .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(error.message().contains("svg.scoped_css"), "{error:?}");
        assert!(error.message().contains("trusted Rust or native CLI host"));
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
        let pipeline = request::pipeline_for_options(&options).unwrap();
        let out = pipeline
            .process_to_string(
                r#"<svg id="host" style="max-width: 400px; background-color: white;"><g/></svg>"#,
                &render_session(),
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
    fn reusable_request_overlay_rejects_camel_case_css_override_policy() {
        let engine = crate::BindingEngine::new(b"").unwrap();
        let error = engine
            .execute(
                crate::BindingOperationRequest::new("svg", b"flowchart TD\nA[Hello]")
                    .with_options_json(br#"{"svg":{"cssOverridePolicy":"preserve"}}"#),
            )
            .unwrap_err();

        assert_eq!(error.status(), BindingStatus::OptionsJsonError);
        assert!(
            error.message().contains("svg.cssOverridePolicy"),
            "{error:?}"
        );
        assert!(error.message().contains("trusted Rust or native CLI host"));
    }

    #[test]
    fn readable_svg_options_can_drop_native_duplicate_fallbacks() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
<text class="task">Make tea</text>
<g transform="translate(0,0)">
  <foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Make tea</p></div></foreignObject>
</g>
<g transform="translate(0,40)">
  <foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Only fallback</p></div></foreignObject>
</g>
</svg>"##;

        let cleanup_options = parse_options(
            br#"{"svg":{"pipeline":"readable","drop_native_duplicate_fallbacks":true}}"#,
        )
        .unwrap();
        let cleanup_pipeline = request::pipeline_for_options(&cleanup_options).unwrap();
        let cleanup_out = cleanup_pipeline
            .process_to_string(svg, &render_session())
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
        assert!(cleanup_out.contains("<foreignObject"));
    }

    #[test]
    fn resvg_safe_svg_options_can_drop_native_duplicate_fallbacks() {
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
        let default_pipeline = request::pipeline_for_options(&default_options).unwrap();
        let default_out = default_pipeline
            .process_to_string(svg, &render_session())
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
        let cleanup_pipeline = request::pipeline_for_options(&cleanup_options).unwrap();
        let cleanup_out = cleanup_pipeline
            .process_to_string(svg, &render_session())
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
            &crate::parse_json(b"flowchart TD\nA[Hello] --> B[World]", b"").unwrap(),
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
        let json: Value =
            serde_json::from_slice(&crate::parse_json(source, options).unwrap()).unwrap();

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
            let err = crate::parse_json(b"flowchart TD\nA[Hello]", options).unwrap_err();

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
    fn edge_geometry_json_returns_post_paint_edge_geometry() {
        let json: Value = serde_json::from_slice(
            &edge_geometry_json(b"flowchart TD\nA[Hello] --> B[World]", b"").unwrap(),
        )
        .unwrap();

        assert_eq!(json["schema_version"], 1);
        assert!(json["diagram_type"].as_str().is_some_and(|t| !t.is_empty()));

        let edges = json["edges"].as_array().expect("edges array");
        assert!(!edges.is_empty());
        assert!(edges.iter().all(|edge| edge["id"].as_str().is_some()));
    }

    #[cfg(feature = "analysis")]
    #[test]
    fn validate_json_reports_success_and_errors_without_throwing() {
        let valid: Value =
            serde_json::from_slice(&crate::validate_json(b"flowchart TD\nA[Hello]", b"").unwrap())
                .unwrap();
        assert_eq!(valid["valid"], true);
        assert_eq!(valid["code_name"], BindingStatus::Ok.code_name());
        assert_eq!(valid.get("error"), Some(&Value::Null));

        let invalid: Value =
            serde_json::from_slice(&crate::validate_json(b"", b"").unwrap()).unwrap();
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
        for (options, field) in [
            (
                br#"{ "layout": { "container_width": -1 } }"#.as_slice(),
                "layout.container_width",
            ),
            (
                br#"{ "layout": { "screen_available_width": 0 } }"#.as_slice(),
                "layout.screen_available_width",
            ),
        ] {
            let err = render_svg(b"flowchart TD\nA", options).unwrap_err();

            assert_eq!(err.status(), BindingStatus::InvalidArgument);
            assert!(err.message().contains(field), "{err:?}");
        }
    }

    #[test]
    fn resource_limit_error_uses_dedicated_binding_status() {
        let err = render_svg(
            b"flowchart TD\nA[Hello]",
            br#"{ "resources": { "limits": { "max_source_bytes": 4 } } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::ResourceLimitExceeded);
        assert!(err.message().contains("max_source_bytes"), "{err:?}");
    }

    #[test]
    fn parse_json_source_limit_uses_dedicated_binding_status() {
        let err = crate::parse_json(
            b"flowchart TD\nA[Hello]",
            br#"{ "resources": { "limits": { "max_source_bytes": 4 } } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::ResourceLimitExceeded);
        assert!(err.message().contains("max_source_bytes"), "{err:?}");
    }

    #[test]
    fn resource_limit_error_accepts_analysis_wrapper_options() {
        let err = render_svg(
            b"flowchart TD\nA[Hello]",
            br#"{ "analysis": { "resources": { "limits": { "max_source_bytes": 4 } } } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::ResourceLimitExceeded);
        assert!(err.message().contains("max_source_bytes"), "{err:?}");
    }

    #[test]
    fn invalid_resource_options_return_invalid_argument() {
        let err = render_svg(
            b"flowchart TD\nA[Hello]",
            br#"{ "resources": { "profile": "unsafe-fast" } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("resources.profile"), "{err:?}");

        let err = render_svg(
            b"flowchart TD\nA[Hello]",
            br#"{ "resources": { "limits": { "max_svg_bytes": 0 } } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("max_svg_bytes"), "{err:?}");

        let err = render_svg(
            b"flowchart TD\nA[Hello]",
            br#"{ "resources": { "limits": { "max_layout_work_units": 0 } } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("max_layout_work_units"), "{err:?}");

        for (id, expected) in [
            ("future_limit", "not part of resource contract"),
            ("max_svg_tree_depth", "not part of resource contract"),
        ] {
            let options = format!(r#"{{ "resources": {{ "limits": {{ "{id}": 8 }} }} }}"#);
            let err = render_svg(b"flowchart TD\nA[Hello]", options.as_bytes()).unwrap_err();
            assert_eq!(err.status(), BindingStatus::InvalidArgument);
            assert!(err.message().contains(expected), "{err:?}");
        }
    }

    #[test]
    fn venn_private_pairwise_expansion_uses_the_binding_resource_budget() {
        let err = render_svg(
            b"venn-beta\nset A\nset B\nset C\nset D\n",
            br#"{ "resources": { "limits": { "max_layout_work_units": 6 } } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::ResourceLimitExceeded);
        assert!(err.message().contains("max_layout_work_units"), "{err:?}");
    }

    #[test]
    fn superseded_enum_value_aliases_are_rejected() {
        let cases = [
            (
                r#"{ "resources": { "profile": "typst_package" } }"#,
                "resources.profile",
            ),
            (
                r#"{ "resources": { "profile": "typst" } }"#,
                "resources.profile",
            ),
            (
                r#"{ "resources": { "profile": "trusted_native" } }"#,
                "resources.profile",
            ),
            (
                r#"{ "resources": { "profile": "trusted" } }"#,
                "resources.profile",
            ),
            (
                r#"{ "resources": { "profile": "unbounded_for_trusted_input" } }"#,
                "resources.profile",
            ),
            (
                r#"{ "resources": { "profile": "unbounded" } }"#,
                "resources.profile",
            ),
            (r#"{ "svg": { "pipeline": "resvg_safe" } }"#, "svg.pipeline"),
            (
                r#"{ "theme": { "preset": "editor_light" } }"#,
                "theme.preset",
            ),
            (r#"{ "theme": { "preset": "onedark" } }"#, "theme.preset"),
            (
                r##"{
                    "theme": {
                        "spec": {
                            "styles": [{
                                "kind": "rule",
                                "target": "node_label",
                                "style": { "fill": "#fff" }
                            }]
                        }
                    }
                }"##,
                "theme.spec.styles.target",
            ),
        ];

        for (options, field) in cases {
            let err = render_svg(b"flowchart TD\nA[Hello]", options.as_bytes()).unwrap_err();
            assert_eq!(err.status(), BindingStatus::InvalidArgument, "{options}");
            assert!(err.message().contains(field), "{options}: {err:?}");
        }
    }

    #[test]
    fn invalid_text_measurement_profile_returns_invalid_argument() {
        let err = render_svg(
            b"flowchart TD\nA[Hello]",
            br#"{ "environment": { "text_measurement": "typst-font-assets" } }"#,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("environment.text_measurement"));
        assert!(err.message().contains("typst-font-assets"));
    }

    #[test]
    fn removed_layout_fields_are_rejected_with_their_migration_target() {
        for (legacy_field, replacement) in [
            (
                r#"{ "layout": { "text_measurer": "deterministic" } }"#,
                "environment.text_measurement",
            ),
            (
                r#"{ "layout": { "math_renderer": "ratex" } }"#,
                "environment.math_renderer",
            ),
            (
                r#"{ "layout": { "viewport_width": 640 } }"#,
                "layout.container_width",
            ),
            (
                r#"{ "layout": { "viewport_height": 480 } }"#,
                "layout.container_height",
            ),
        ] {
            let err = render_svg(b"flowchart TD\nA[Hello]", legacy_field.as_bytes()).unwrap_err();

            assert_eq!(err.status(), BindingStatus::OptionsJsonError);
            assert!(err.message().contains(replacement), "{err:?}");
        }
    }

    #[test]
    fn ratex_selection_follows_the_artifact_owner_math_feature() {
        let result = render_svg(
            b"flowchart TD\nA[Hello]",
            br#"{ "environment": { "math_renderer": "ratex" } }"#,
        );

        if cfg!(feature = "math") {
            assert!(result.is_ok(), "{result:?}");
        } else {
            let err = result.unwrap_err();
            assert_eq!(err.status(), BindingStatus::UnsupportedOperation);
            assert_eq!(err.capability_id(), Some("math"));
        }
    }
}
