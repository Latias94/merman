use super::SvgPostprocessExecution;
use crate::Result;
use crate::xml_dom::{Document, NodeId};
use std::borrow::Cow;
use std::collections::HashSet;

pub(super) fn validate(
    document: &Document<'_>,
    execution: SvgPostprocessExecution<'_>,
) -> Result<HashSet<NodeId>> {
    merman_render_core::step_animation::validate(document, &mut || execution.checkpoint())
}

pub(super) fn project_static<'a>(
    svg: Cow<'a, str>,
    execution: SvgPostprocessExecution<'_>,
) -> Result<Cow<'a, str>> {
    merman_render_core::step_animation::project_static(svg, &mut || execution.checkpoint())
}

#[cfg(test)]
mod tests {
    use crate::svg::SvgPipeline;
    use merman_render_core::step_animation::animation_id;

    fn fixture() -> String {
        let motion = animation_id("diagram-edge-step");
        let transform = format!(
            r#"<animateTransform attributeName="transform" type="scale" values="1;1" dur="0.25s" begin="{motion}.begin" fill="remove"/>"#
        );
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" id="diagram"><g class="edgePaths"><path id="diagram-edge" data-edge="true" d="M0,0L100,0"/><path class="edge-step-highlight" aria-hidden="true" d="M0,0L100,0" fill="none" stroke="#333333" stroke-width="4" stroke-linecap="round" stroke-opacity="0.6" pointer-events="none" transform="translate(0,0) scale(0)" data-step="3">{transform}</path><circle class="edge-step-dot" r="4" aria-hidden="true" transform="translate(0,0) scale(0)" data-step="3" fill="#333333" stroke="none"><animateMotion id="{motion}" path="M0,0L100,0" dur="0.25s" begin="0.5s;{motion}.end+1s" fill="remove"/>{transform}</circle></g><g class="edge-step-badges"><g class="edge-step-badge"><circle r="9"/><text>3</text></g></g></svg>"##
        )
    }

    #[test]
    fn browser_admission_and_static_projection_are_distinct_closed_contracts() {
        let source = fixture();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let browser = SvgPipeline::parity()
            .with_browser_inline_contract("embed-step_bad")
            .process_to_string(&source, &session)
            .unwrap();
        assert!(browser.contains("animateMotion"));
        super::super::validate_static_inline_svg_admission(&browser, &session).unwrap();
        assert!(super::super::validate_static_inline_svg(&browser, &session).is_err());
        for pipeline in [
            SvgPipeline::resvg_safe(),
            SvgPipeline::parity().with_static_inline_contract("static"),
        ] {
            let output = pipeline.process_to_string(&source, &session).unwrap();
            assert!(!output.contains("animateMotion"));
            assert!(!output.contains("edge-step-highlight"));
            assert!(!output.contains("edge-step-dot"));
            assert!(output.contains("edge-step-badge"));
            assert!(output.contains("data-edge=\"true\""));
        }
    }

    #[test]
    fn static_step_projection_handles_nested_style_ranges_without_panicking() {
        let source = r#"<svg xmlns="http://www.w3.org/2000/svg"><style class="edge-step-style"><style class="edge-step-style"/></style></svg>"#;
        assert_eq!(source.len(), 117);
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        for pipeline in [
            SvgPipeline::resvg_safe(),
            SvgPipeline::parity().with_static_inline_contract("static"),
        ] {
            let output = pipeline.process_to_string(source, &session).unwrap();
            assert!(!output.contains("edge-step-style"));
            super::super::validate_static_inline_svg(&output, &session).unwrap();
        }
    }

    #[test]
    fn static_step_projection_preserves_siblings_and_rejects_invalid_nested_content() {
        let styles = r#"<style class="edge-step-style"><style class="edge-step-style"/></style>"#;
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg"><path id="before"/>{styles}<text>after</text></svg>"#
        );
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        for pipeline in [
            SvgPipeline::resvg_safe(),
            SvgPipeline::parity().with_static_inline_contract("static"),
        ] {
            let output = pipeline.process_to_string(&source, &session).unwrap();
            assert!(output.contains("before"));
            assert!(output.contains(">after</text>"));
            assert!(!output.contains("edge-step-style"));
            for invalid in [
                source.replace("</svg>", ""),
                source.replace(
                    styles,
                    r##"<style class="edge-step-style"><animateMotion href="#before"/></style>"##,
                ),
            ] {
                assert!(pipeline.process_to_string(&invalid, &session).is_err());
            }
        }
    }

    #[test]
    fn browser_step_admission_handles_deep_parallel_edges_and_rejects_nested_svg() {
        let source = fixture();
        let edges = source
            .split_once("<g class=\"edgePaths\">")
            .unwrap()
            .1
            .split_once("</g>")
            .unwrap()
            .0;
        let motion = animation_id("diagram-edge-step");
        let mut parallel = String::new();
        for index in 0..128 {
            let edge_id = format!("diagram-edge{index}");
            parallel.push_str(
                &edges
                    .replace("diagram-edge", &edge_id)
                    .replace(&motion, &animation_id(&format!("{edge_id}-step"))),
            );
        }
        let nested = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" id="diagram">{}<g class="edgePaths">{parallel}</g>{}</svg>"#,
            "<g>".repeat(2048),
            "</g>".repeat(2048)
        );
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        super::super::validate_static_inline_svg_admission(&nested, &session).unwrap();
        for (open, close) in [
            ("<svg>", "</svg>"),
            (
                r#"<s:svg xmlns:s="http://www.w3.org/2000/svg">"#,
                "</s:svg>",
            ),
            ("<foreignObject>", "</foreignObject>"),
        ] {
            let invalid = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" id="diagram">{open}<g class="edgePaths">{parallel}</g>{close}</svg>"#
            );
            assert!(
                super::super::validate_static_inline_svg_admission(&invalid, &session).is_err()
            );
        }
    }

    #[test]
    fn step_decimal_timing_accepts_large_counts_and_rejects_fractional_cycles() {
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let pipeline = SvgPipeline::parity().with_browser_inline_contract("embed");
        for (step, begin) in [("1", "0s"), ("999999999", "99999999.80000001s")] {
            let source = fixture()
                .replace("data-step=\"3\"", &format!("data-step=\"{step}\""))
                .replace("dur=\"0.25s\"", "dur=\"0.1s\"")
                .replace("begin=\"0.5s;", &format!("begin=\"{begin};"))
                .replace(".end+1s", ".end+99999999.80000001s");
            pipeline.process_to_string(&source, &session).unwrap();
            let fractional = source.replace(".end+99999999.80000001s", ".end+99999999.825s");
            assert!(pipeline.process_to_string(&fractional, &session).is_err());
            if step == "999999999" {
                let early = source.replace(".end+99999999.80000001s", ".end+99999999.7s");
                assert!(pipeline.process_to_string(&early, &session).is_err());
            }
        }
        let distant = fixture()
            .replace("data-step=\"3\"", "data-step=\"10000000000000000\"")
            .replace("dur=\"0.25s\"", "dur=\"0.1s\"")
            .replace("begin=\"0.5s;", "begin=\"1000000000000000s;")
            .replace(".end+1s", ".end+1000000000000000s");
        pipeline.process_to_string(&distant, &session).unwrap();
        let early = distant.replace(".end+1000000000000000s", ".end+999999999999999.8s");
        assert!(pipeline.process_to_string(&early, &session).is_err());
    }

    #[test]
    fn step_rejects_matching_malformed_and_nonfinite_paths() {
        let source = fixture();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        let pipeline = SvgPipeline::parity().with_browser_inline_contract("embed");
        pipeline.process_to_string(&source, &session).unwrap();
        for path in [
            "javascript:alert(1)",
            "M0,0L1e999,0",
            "M0,0L1",
            "M0,0A2,2,0,2,0,4,4",
        ] {
            let invalid = source.replace("M0,0L100,0", path);
            assert_ne!(invalid, source);
            assert!(
                pipeline.process_to_string(&invalid, &session).is_err(),
                "{path}"
            );
        }
    }

    #[test]
    fn step_rejects_active_targeting_timing_and_path_variants() {
        let source = fixture();
        let session = crate::environment::RenderEnvironment::deterministic()
            .begin_session()
            .unwrap();
        for (from, to) in [
            ("attributeName=\"transform\"", "attributeName=\"href\""),
            ("type=\"scale\"", "type=\"translate\""),
            ("values=\"1;1\"", "values=\"1;100\""),
            ("<animateMotion ", "<animateMotion href=\"#other\" "),
            ("<animateMotion ", "<animateMotion onbegin=\"alert(1)\" "),
            (
                "<animateMotion ",
                "<animateMotion repeatCount=\"indefinite\" ",
            ),
            ("path=\"M0,0L100,0\"", "path=\"javascript:alert(1)\""),
            ("path=\"M0,0L100,0\"", "path=\"M0,0L1e999,0\""),
            ("begin=\"0.5s;", "begin=\"click;"),
            ("begin=\"0.5s;", "begin=\"0.6s;"),
            (".end+1s", ".end+0.1s"),
            (".begin\"", ".click\""),
            ("dur=\"0.25s\"", "dur=\"0s\""),
            ("fill=\"remove\"", "fill=\"freeze\""),
            ("id=\"diagram\"", "id=\"other\""),
            ("class=\"edgePaths\"", "class=\"other\""),
            ("</svg>", "<animateMotion path=\"M0,0L1,1\"/></svg>"),
            ("data-edge=\"true\"", "data-edge=\"false\""),
        ] {
            let attack = source.replacen(from, to, 1);
            assert!(
                SvgPipeline::parity()
                    .with_browser_inline_contract("embed")
                    .process_to_string(&attack, &session)
                    .is_err(),
                "{from} -> {to}"
            );
        }
    }
}
