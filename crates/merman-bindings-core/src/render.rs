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

#[derive(Clone)]
pub(crate) struct CachedRenderEngine {
    plan: RenderRequestPlan,
}

pub(crate) struct RenderOperationConfig {
    plan: request::RenderOperationConfig,
}

impl CachedRenderEngine {
    pub(crate) fn render_svg(
        &self,
        source: &[u8],
        control: merman::OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        let source = source_text(source)?;
        self.plan.render_svg(source, control)
    }

    pub(crate) fn layout_json(
        &self,
        source: &[u8],
        control: merman::OperationControl,
    ) -> Result<Vec<u8>, BindingError> {
        let source = source_text(source)?;
        self.plan.layout_json(source, control)
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
    ) -> Result<Self, BindingError> {
        Ok(Self {
            plan: request::RenderOperationConfig::compile(
                options,
                runtime_policy,
                capability_policy,
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
        assert_eq!(
            svg.matches("<style").count(),
            1,
            "site CSS should merge into the existing Mermaid stylesheet: {svg}"
        );
    }

    #[test]
    fn render_svg_accepts_presentation_theme_and_independent_svg_output() {
        let options = br##"{
            "presentation": {
                "theme": {
                    "appearance": "dark",
                    "font_family": "system-ui",
                    "roles": {
                        "canvas": "#0f172a",
                        "surface": "#111827",
                        "text": "#e5e7eb",
                        "border": "#475569",
                        "line": "#94a3b8",
                        "note-background": "#422006",
                        "note-border": "#f59e0b"
                    },
                    "series_palette": ["#60a5fa", "#34d399", "#f59e0b"]
                }
            },
            "svg": {
                "diagram_id": "bindings host theme",
                "pipeline": "resvg-safe",
                "root_background_color": "#0f172a",
                "drop_native_duplicate_fallbacks": true,
                "css_override_policy": "strip-existing-important"
            }
        }"##;
        let svg = String::from_utf8(
            render_svg(
                b"sequenceDiagram\n  participant A as Alpha\n  participant B as Beta\n  A->>B: Hello\n  Note over A,B: Host note",
                options,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(svg.contains(r#"id="bindings-host-theme""#), "{svg}");
        assert!(svg.contains("#111827"), "{svg}");
        assert!(svg.contains("#e5e7eb"), "{svg}");
        assert!(svg.contains("#94a3b8"), "{svg}");
        assert!(svg.contains("#422006"), "{svg}");
        assert!(svg.contains("#f59e0b"), "{svg}");
        assert!(
            root_style_property_is(&svg, "background-color", "#0f172a"),
            "{svg}"
        );
        assert!(!svg.contains("<foreignObject"), "{svg}");
        assert!(!svg.contains("!important"), "{svg}");
    }

    #[test]
    fn explicit_site_config_overrides_presentation_theme_variables() {
        let options = br##"{
            "presentation": {
                "theme": {
                    "roles": {
                        "surface": "#111111",
                        "text": "#eeeeee",
                        "border": "#222222"
                    }
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
        assert!(svg.contains("#eeeeee"), "{svg}");
    }

    #[test]
    fn empty_presentation_is_noop_for_theme_config() {
        let plain = String::from_utf8(
            render_svg(
                b"flowchart TD\nA[Host]",
                br##"{ "svg": { "diagram_id": "bindings empty host theme" } }"##,
            )
            .unwrap(),
        )
        .unwrap();
        let themed = String::from_utf8(
            render_svg(
                b"flowchart TD\nA[Host]",
                br##"{
                    "presentation": {},
                    "svg": { "diagram_id": "bindings empty host theme" }
                }"##,
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            themed, plain,
            "empty presentation should not force theme=base or mutate SVG output"
        );
    }

    #[test]
    fn presentation_theme_preset_applies_common_editor_theme() {
        let svg = String::from_utf8(
            render_svg(
                b"flowchart TD\nA[One Dark] --> B[Readable]",
                br##"{
                    "presentation": {
                        "theme": { "preset": "one-dark" }
                    },
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
    fn json_presentation_matches_the_equivalent_rust_presentation() {
        let source = "flowchart TD\nA[One Dark] --> B[Modern]";
        let options = br##"{
            "presentation": {
                "profile": "merman-modern",
                "theme": { "preset": "one-dark" }
            },
            "site_config": {
                "flowchart": { "defaultRenderer": "dagre-wrapper" }
            },
            "svg": { "diagram_id": "presentation equivalence" }
        }"##;
        let binding_svg = String::from_utf8(render_svg(source.as_bytes(), options).unwrap())
            .expect("binding SVG should be UTF-8");

        let presentation = merman::svg::Presentation::new()
            .with_profile(merman::svg::PresentationProfile::MermanModern)
            .with_theme(merman::svg::HostTheme::from_preset(
                merman::svg::HostThemePreset::OneDark,
            ))
            .resolve();
        let engine = presentation
            .materialize_engine(merman::Engine::new())
            .with_site_config(merman::MermaidConfig::from_value(serde_json::json!({
                "flowchart": { "defaultRenderer": "dagre-wrapper" },
            })));
        let svg_request = merman::SvgRequest {
            options: merman::svg::SvgRenderOptions {
                diagram_id: Some("presentation equivalence".to_string()),
                ..Default::default()
            },
            presentation: presentation.render_policy(),
            ..Default::default()
        };
        let renderer = merman::Renderer::new().with_engine(engine);
        let rust_svg = renderer
            .render(merman::RenderRequest::svg(
                source,
                merman::OperationControl::new(),
                svg_request.clone(),
            ))
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
            .render(merman::RenderRequest::svg_plan(
                source,
                merman::OperationControl::new(),
                svg_request,
            ))
            .expect("Rust planning should succeed");
        let merman::RenderOutput::SvgPlan(Some(rust_plan)) = rust_plan else {
            panic!("typed SVG-plan request should produce a capability plan");
        };
        let rust_aspects = rust_plan
            .presentation_aspects()
            .iter()
            .map(|aspect| {
                serde_json::json!({
                    "id": aspect.id(),
                    "state": aspect.state().as_str(),
                    "required_capability_id": aspect.required_capability_id(),
                })
            })
            .collect::<Vec<_>>();

        assert_eq!(
            binding_plan["presentation_profile_id"],
            serde_json::json!(rust_plan.presentation_profile_id())
        );
        assert_eq!(
            binding_plan["presentation_aspects"],
            serde_json::json!(rust_aspects)
        );
        assert_eq!(binding_plan["ready"], rust_plan.is_ready());
    }

    #[test]
    fn modern_profile_defers_elk_availability_to_flowchart_admission() {
        let profile = br##"{ "presentation": { "profile": "merman-modern" } }"##;
        let sequence = render_svg(b"sequenceDiagram\nA->>B: Hello", profile);
        assert!(
            sequence.is_ok(),
            "non-Flowchart families do not require ELK"
        );

        let dagre = render_svg(
            b"flowchart TD\nA --> B",
            br##"{
                "presentation": { "profile": "merman-modern" },
                "site_config": {
                    "flowchart": { "defaultRenderer": "dagre-wrapper" }
                }
            }"##,
        );
        assert!(
            dagre.is_ok(),
            "an explicit available renderer should override the profile default"
        );

        let default_flowchart = render_svg(b"flowchart TD\nA --> B", profile);
        if cfg!(feature = "layout-elk") {
            assert!(default_flowchart.is_ok());
        } else {
            let error = default_flowchart.expect_err("missing ELK should block admission");
            assert_eq!(error.status(), BindingStatus::UnsupportedOperation);
            assert_eq!(error.kind(), crate::BindingErrorKind::MissingCapability);
            assert_eq!(error.capability_id(), Some("layout-elk"));
        }
    }

    #[cfg(feature = "layout-elk")]
    #[test]
    fn merman_modern_profile_renders_neo_elk_flowcharts() {
        let svg = String::from_utf8(
            render_svg(
                b"flowchart LR\nA[Start] -->|Continue| B[Finish]",
                br##"{
                    "presentation": { "profile": "merman-modern" },
                    "svg": { "diagram_id": "bindings merman modern" }
                }"##,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(svg.contains(r#"data-look="neo""#), "{svg}");
        assert!(svg.contains(r#"rx="12" ry="12""#), "{svg}");
        assert!(
            svg.contains("fill:#F8FAFC;stroke:#64748B;stroke-width:2px;"),
            "{svg}"
        );
        assert!(svg.contains("stroke:#64748B"), "{svg}");
        assert!(
            svg.contains("stroke-linecap:round;stroke-linejoin:round;"),
            "{svg}"
        );
        assert!(svg.contains(".edgeLabel rect{opacity:1;}"), "{svg}");
        assert!(svg.contains(r#"rx="4" ry="4""#), "{svg}");
        assert!(svg.contains("bindings-merman-modern-drop-shadow"), "{svg}");
    }

    #[test]
    fn presentation_theme_preset_allows_role_overrides() {
        let svg = String::from_utf8(
            render_svg(
                b"flowchart TD\nA[Override]",
                br##"{
                    "presentation": {
                        "theme": {
                            "preset": "ayu-dark",
                            "roles": {
                                "canvas": "#101010",
                                "line": "#ff00aa"
                            }
                        }
                    },
                    "svg": {
                        "diagram_id": "bindings ayu override",
                        "root_background_color": "#101010"
                    }
                }"##,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(svg.contains("#101010"), "{svg}");
        assert!(svg.contains("#ff00aa"), "{svg}");
        assert!(svg.contains("#bfbdb6"), "{svg}");
        assert!(
            root_style_property_is(&svg, "background-color", "#101010"),
            "{svg}"
        );
    }

    #[test]
    fn invalid_presentation_theme_preset_returns_invalid_argument() {
        let err = render_svg(
            b"flowchart TD\nA[Host]",
            br##"{ "presentation": { "theme": { "preset": "solarized-maybe" } } }"##,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("presentation.theme.preset"));
    }

    #[test]
    fn invalid_presentation_profile_and_role_return_invalid_argument() {
        for (options, field) in [
            (
                br##"{ "presentation": { "profile": "future-modern" } }"##.as_slice(),
                "presentation.profile",
            ),
            (
                br##"{ "presentation": { "theme": { "roles": { "future-role": "#fff" } } } }"##
                    .as_slice(),
                "presentation.theme.roles",
            ),
        ] {
            let err = render_svg(b"flowchart TD\nA[Host]", options).unwrap_err();
            assert_eq!(err.status(), BindingStatus::InvalidArgument);
            assert!(err.message().contains(field), "{err:?}");
        }
    }

    #[test]
    fn presentation_rejects_null_and_mixed_owner_fields() {
        for (options, field) in [
            (r#"{ "presentation": null }"#, "presentation"),
            (
                r#"{ "presentation": { "theme": { "output": {} } } }"#,
                "output",
            ),
            (
                r#"{ "presentation": { "theme": { "theme_variables": {} } } }"#,
                "theme_variables",
            ),
            (
                r#"{ "presentation": { "theme": { "site_config": {} } } }"#,
                "site_config",
            ),
        ] {
            let err = render_svg(b"flowchart TD\nA[Host]", options.as_bytes()).unwrap_err();
            assert_eq!(err.status(), BindingStatus::OptionsJsonError);
            assert!(err.message().contains(field), "{err:?}");
        }
    }

    #[test]
    fn removed_host_theme_reports_the_presentation_migration() {
        let err = render_svg(
            b"flowchart TD\nA[Host]",
            br##"{ "host_theme": { "preset": "one-dark" } }"##,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::OptionsJsonError);
        assert!(err.message().contains("presentation.profile"));
        assert!(err.message().contains("presentation.theme"));
        assert!(err.message().contains("site_config"));
        assert!(err.message().contains("svg"));
    }

    #[test]
    fn svg_css_override_policy_is_owned_only_by_svg_options() {
        let options = parse_options(
            br##"{
                "svg": {
                    "pipeline": "resvg-safe",
                    "css_override_policy": "preserve"
                }
            }"##,
        )
        .unwrap();
        let pipeline = request::pipeline_for_options(&options).unwrap();
        let out = pipeline
            .process_to_string(
                r#"<svg id="host"><style>.node{fill:red !important;}</style></svg>"#,
                &render_session(),
            )
            .unwrap();

        assert!(
            out.contains("!important"),
            "svg.css_override_policy=preserve should retain existing important declarations: {out}"
        );
    }

    #[test]
    fn invalid_presentation_theme_color_returns_invalid_argument() {
        let err = render_svg(
            b"flowchart TD\nA[Host]",
            br##"{ "presentation": { "theme": { "roles": { "canvas": "white; color: red" } } } }"##,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("presentation.theme.roles.canvas"));
    }

    #[test]
    fn invalid_presentation_theme_success_color_returns_invalid_argument() {
        let err = render_svg(
            b"flowchart TD\nA[Host]",
            br##"{ "presentation": { "theme": { "roles": { "success": "#00ff00; color: red" } } } }"##,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("presentation.theme.roles.success"));
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
        let pipeline = request::pipeline_for_options(&options).unwrap();
        let out = pipeline
            .process_to_string(
                r#"<svg id="host"><style>.node{fill:red !important;}</style><g/></svg>"#,
                &render_session(),
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
        let pipeline = request::pipeline_for_options(&options).unwrap();
        let out = pipeline
            .process_to_string(
                r#"<svg id="host"><path class="edge"/></svg>"#,
                &render_session(),
            )
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
                r#"{ "svg": { "css_override_policy": "strip_existing_important" } }"#,
                "svg.css_override_policy",
            ),
            (
                r#"{ "presentation": { "theme": { "preset": "editor_light" } } }"#,
                "presentation.theme.preset",
            ),
            (
                r#"{ "presentation": { "theme": { "preset": "onedark" } } }"#,
                "presentation.theme.preset",
            ),
            (
                r##"{ "presentation": { "theme": { "roles": { "surface_alt": "#fff" } } } }"##,
                "presentation.theme.roles",
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
