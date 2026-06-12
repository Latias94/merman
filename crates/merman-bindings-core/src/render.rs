mod request;

use crate::common::{
    BindingError, BindingOptions, parse_options, source_text, validation_payload_json,
};
use request::RenderRequestPlan;

pub fn render_svg(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = source_text(source)?;
    request_plan_from_options_json(options_json)?.render_svg(source)
}

pub fn parse_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = source_text(source)?;
    request_plan_from_options_json(options_json)?.parse_json(source)
}

pub fn layout_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    let source = source_text(source)?;
    request_plan_from_options_json(options_json)?.layout_json(source)
}

pub fn validate_json(source: &[u8], options_json: &[u8]) -> Result<Vec<u8>, BindingError> {
    validation_payload_json(validate_source(source, options_json))
}

#[derive(Clone)]
pub(crate) struct CachedRenderEngine {
    plan: RenderRequestPlan,
}

impl CachedRenderEngine {
    pub(crate) fn new(options: &BindingOptions) -> Result<Self, BindingError> {
        Ok(Self {
            plan: RenderRequestPlan::from_options(options)?,
        })
    }

    pub(crate) fn render_svg(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        let source = source_text(source)?;
        self.plan.render_svg(source)
    }

    pub(crate) fn parse_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        let source = source_text(source)?;
        self.plan.parse_json(source)
    }

    pub(crate) fn layout_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        let source = source_text(source)?;
        self.plan.layout_json(source)
    }

    pub(crate) fn validate_json(&self, source: &[u8]) -> Result<Vec<u8>, BindingError> {
        validation_payload_json(self.validate_source(source))
    }

    fn validate_source(&self, source: &[u8]) -> Result<(), BindingError> {
        let source = source_text(source)?;
        self.plan.validate(source)
    }
}

fn request_plan_from_options_json(options_json: &[u8]) -> Result<RenderRequestPlan, BindingError> {
    let options = parse_options(options_json)?;
    RenderRequestPlan::from_options(&options)
}

fn validate_source(source: &[u8], options_json: &[u8]) -> Result<(), BindingError> {
    let source = source_text(source)?;
    request_plan_from_options_json(options_json)?.validate(source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BindingStatus;
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
    fn render_svg_accepts_host_theme_profile() {
        let options = br##"{
            "host_theme": {
                "appearance": "dark",
                "font_family": "system-ui",
                "roles": {
                    "canvas": "#0f172a",
                    "surface": "#111827",
                    "text": "#e5e7eb",
                    "border": "#475569",
                    "line": "#94a3b8",
                    "note_background": "#422006",
                    "note_border": "#f59e0b"
                },
                "series_palette": ["#60a5fa", "#34d399", "#f59e0b"],
                "output": {
                    "pipeline": "resvg-safe",
                    "root_background": "canvas",
                    "drop_native_duplicate_fallbacks": true,
                    "css_override_policy": "strip-existing-important"
                }
            },
            "svg": { "diagram_id": "bindings host theme" }
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
        assert!(svg.contains("background-color: #0f172a;"), "{svg}");
        assert!(!svg.contains("<foreignObject"), "{svg}");
        assert!(!svg.contains("!important"), "{svg}");
    }

    #[test]
    fn explicit_site_config_overrides_host_theme_profile_variables() {
        let options = br##"{
            "host_theme": {
                "roles": {
                    "surface": "#111111",
                    "text": "#eeeeee",
                    "border": "#222222"
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
    fn empty_host_theme_profile_is_noop_for_theme_config() {
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
                    "host_theme": {},
                    "svg": { "diagram_id": "bindings empty host theme" }
                }"##,
            )
            .unwrap(),
        )
        .unwrap();

        assert_eq!(
            themed, plain,
            "empty host_theme should not force theme=base or mutate SVG output"
        );
    }

    #[test]
    fn host_theme_preset_applies_common_editor_theme() {
        let svg = String::from_utf8(
            render_svg(
                b"flowchart TD\nA[One Dark] --> B[Readable]",
                br##"{
                    "host_theme": {
                        "preset": "one-dark"
                    },
                    "svg": { "diagram_id": "bindings one dark" }
                }"##,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(svg.contains("#282c34"), "{svg}");
        assert!(svg.contains("#abb2bf"), "{svg}");
        assert!(svg.contains("#61afef"), "{svg}");
        assert!(svg.contains("background-color: #282c34;"), "{svg}");
    }

    #[test]
    fn host_theme_preset_allows_role_overrides() {
        let svg = String::from_utf8(
            render_svg(
                b"flowchart TD\nA[Override]",
                br##"{
                    "host_theme": {
                        "preset": "ayu-dark",
                        "roles": {
                            "canvas": "#101010",
                            "line": "#ff00aa"
                        }
                    },
                    "svg": { "diagram_id": "bindings ayu override" }
                }"##,
            )
            .unwrap(),
        )
        .unwrap();

        assert!(svg.contains("#101010"), "{svg}");
        assert!(svg.contains("#ff00aa"), "{svg}");
        assert!(svg.contains("#bfbdb6"), "{svg}");
        assert!(svg.contains("background-color: #101010;"), "{svg}");
    }

    #[test]
    fn invalid_host_theme_preset_returns_invalid_argument() {
        let err = render_svg(
            b"flowchart TD\nA[Host]",
            br##"{ "host_theme": { "preset": "solarized-maybe" } }"##,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("host_theme.preset"));
    }

    #[test]
    fn svg_css_override_policy_can_preserve_after_host_theme_strip_default() {
        let options = parse_options(
            br##"{
                "host_theme": {
                    "appearance": "dark",
                    "output": {
                        "pipeline": "resvg-safe",
                        "css_override_policy": "strip-existing-important"
                    }
                },
                "svg": {
                    "css_override_policy": "preserve"
                }
            }"##,
        )
        .unwrap();
        let pipeline = request::pipeline_for_options(&options).unwrap();
        let out = pipeline
            .process_to_string(r#"<svg id="host"><style>.node{fill:red !important;}</style></svg>"#)
            .unwrap();

        assert!(
            out.contains("!important"),
            "explicit svg.css_override_policy=preserve should override host output stripping: {out}"
        );
    }

    #[test]
    fn invalid_host_theme_color_returns_invalid_argument() {
        let err = render_svg(
            b"flowchart TD\nA[Host]",
            br##"{ "host_theme": { "roles": { "canvas": "white; color: red" } } }"##,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("host_theme.roles.canvas"));
    }

    #[test]
    fn invalid_host_theme_success_color_returns_invalid_argument() {
        let err = render_svg(
            b"flowchart TD\nA[Host]",
            br##"{ "host_theme": { "roles": { "success": "#00ff00; color: red" } } }"##,
        )
        .unwrap_err();

        assert_eq!(err.status(), BindingStatus::InvalidArgument);
        assert!(err.message().contains("host_theme.roles.success"));
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
        let pipeline = request::pipeline_for_options(&options).unwrap();
        let out = pipeline
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
        let cleanup_out = cleanup_pipeline.process_to_string(svg).unwrap();

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
    fn resvg_safe_svg_options_do_not_add_generic_duplicate_cleanup() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg">
<switch>
  <foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Make tea</p></div></foreignObject>
  <text class="task">Make tea</text>
</switch>
<g transform="translate(0,40)">
  <foreignObject width="80" height="24"><div xmlns="http://www.w3.org/1999/xhtml"><p>Only fallback</p></div></foreignObject>
</g>
</svg>"##;

        let default_options = parse_options(br#"{"svg":{"pipeline":"resvg-safe"}}"#).unwrap();
        let default_pipeline = request::pipeline_for_options(&default_options).unwrap();
        let default_out = default_pipeline.process_to_string(svg).unwrap();
        assert_eq!(
            default_out
                .matches(r#"data-merman-foreignobject="fallback""#)
                .count(),
            1,
            "{default_out}"
        );

        let cleanup_options = parse_options(
            br#"{"svg":{"pipeline":"resvg-safe","drop_native_duplicate_fallbacks":true}}"#,
        )
        .unwrap();
        let cleanup_pipeline = request::pipeline_for_options(&cleanup_options).unwrap();
        let cleanup_out = cleanup_pipeline.process_to_string(svg).unwrap();

        assert_eq!(cleanup_out, default_out);

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
