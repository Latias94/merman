#![forbid(unsafe_code)]

use merman::svg::{RenderResourcePolicy, ResourceLimitId, SvgPipeline, TextMeasurementPolicy};
use merman::time::CivilDate;
use merman::{
    Engine, OperationControl, ParseOptions, RenderOutput, RenderRequest, Renderer, SvgEnvironment,
    SvgRequest, runtime::RuntimePolicy,
};
use roxmltree::Document;

pub const MAX_PARSE_INPUT_BYTES: usize = 256 * 1024;
pub const MAX_RENDER_INPUT_BYTES: usize = 32 * 1024;
pub const MAX_SVG_INPUT_BYTES: usize = 256 * 1024;

pub fn bounded_utf8(data: &[u8], max_bytes: usize) -> Option<&str> {
    if data.len() > max_bytes {
        return None;
    }
    std::str::from_utf8(data).ok()
}

pub fn deterministic_engine() -> Engine {
    let policy = RuntimePolicy::deterministic()
        .try_with_fixed_local_offset_minutes(0)
        .expect("valid UTC offset")
        .with_fixed_today(CivilDate::new(2025, 1, 1))
        .with_fixed_unix_millis(1_735_689_600_000);
    Engine::new().with_runtime_policy(policy)
}

#[derive(Debug, Clone)]
pub struct BoundedRenderer {
    renderer: Renderer,
    request: SvgRequest,
}

impl BoundedRenderer {
    pub fn render_resvg_safe_svg(
        &self,
        source: &str,
    ) -> Result<Option<String>, merman::RenderError> {
        let output = self.renderer.render(RenderRequest::svg(
            source,
            OperationControl::new(),
            self.request.clone(),
        ))?;
        let RenderOutput::Svg(output) = output else {
            unreachable!("an SVG request must produce an SVG output variant");
        };
        Ok(output.map(|output| output.into_parts().0))
    }
}

pub fn bounded_renderer() -> BoundedRenderer {
    let mut limits = RenderResourcePolicy::interactive();
    for (id, value) in [
        (ResourceLimitId::MaxSourceBytes, MAX_RENDER_INPUT_BYTES),
        (ResourceLimitId::MaxSvgBytes, 4 * 1024 * 1024),
        (ResourceLimitId::MaxModelItems, 2_048),
        (ResourceLimitId::MaxModelTextBytes, 256 * 1024),
        (ResourceLimitId::MaxLayoutWorkUnits, 250_000),
    ] {
        limits
            .apply_limit(id, value)
            .expect("fuzz resource policy uses overridable positive limits");
    }

    let input_policy = *limits.input_policy();
    let renderer = Renderer::new()
        .with_runtime_policy(
            RuntimePolicy::deterministic().with_fixed_unix_millis(1_735_689_600_000),
        )
        .with_parse_options(ParseOptions::strict())
        .with_resource_policy(input_policy);
    let mut options = SvgRequest::default().options;
    options.diagram_id = Some("fuzz".to_string());
    let request = SvgRequest {
        environment: SvgEnvironment::deterministic()
            .with_text_measurement_policy(TextMeasurementPolicy::deterministic())
            .with_resource_policy(limits),
        options,
        pipeline: Some(SvgPipeline::resvg_safe()),
        ..SvgRequest::default()
    };

    BoundedRenderer { renderer, request }
}

pub fn is_well_formed_svg(svg: &str) -> bool {
    Document::parse(svg).is_ok_and(|document| {
        document
            .root_element()
            .tag_name()
            .name()
            .eq_ignore_ascii_case("svg")
    })
}

pub fn assert_resvg_safe_svg(svg: &str) {
    let session = merman_render::environment::RenderEnvironment::deterministic()
        .begin_session()
        .expect("parity render session must be constructible");
    let normalized = merman_render::svg::finalize_resvg_svg(svg, &session)
        .unwrap_or_else(|error| panic!("resvg-safe output failed terminal validation: {error}"));
    assert_eq!(
        normalized.as_str(),
        svg,
        "resvg-safe output was not terminally normalized"
    );

    let document = Document::parse(svg)
        .unwrap_or_else(|error| panic!("successful SVG output is not well formed: {error}"));
    let root = document.root_element();
    assert!(
        root.tag_name().name().eq_ignore_ascii_case("svg"),
        "successful SVG output has a non-SVG root"
    );

    for node in document.descendants().filter(roxmltree::Node::is_element) {
        let local_name = node.tag_name().name();
        assert!(
            !is_active_svg_element(local_name),
            "resvg-safe output retained active element <{local_name}>"
        );

        for attribute in node.attributes() {
            let name = attribute.name();
            assert!(
                !is_event_handler_attribute(name),
                "resvg-safe output retained event attribute {name}"
            );
        }
    }
}

fn is_active_svg_element(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "script" | "iframe" | "object" | "embed" | "foreignobject"
    )
}

fn is_event_handler_attribute(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.len() > 2
        && name
            .get(..2)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("on"))
        && bytes[2].is_ascii_alphabetic()
}

#[cfg(test)]
mod tests {
    use super::assert_resvg_safe_svg;

    #[test]
    fn css_url_oracle_does_not_treat_function_name_suffixes_as_url_functions() {
        assert_resvg_safe_svg(
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="filter:filterBurl(file:///tmp/x);stroke:#033"/></svg>"##,
        );
    }

    #[test]
    #[should_panic(expected = "resvg-safe output was not terminally normalized")]
    fn terminal_oracle_rejects_an_actual_unsafe_css_url() {
        assert_resvg_safe_svg(
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="fill:url('file:///tmp/x')"/></svg>"##,
        );
    }
}
